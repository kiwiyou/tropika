use crate::handler::*;

#[derive(Clone)]
pub enum CodeLanguage {
    Cpp,
    Bash,
    Python,
    Javascript,
}

#[derive(Clone)]
pub enum CodeSession {
    Real {
        language: code::CodeLanguage,
        code: String,
    },
    Reference {
        id: types::MessageId,
    },
}

enum CodeError {
    Compile(String),
    Runtime(String),
    Other(String),
    Timeout,
}

async fn execute_code(
    language: &CodeLanguage,
    code: &str,
    input: &str,
    timeout: std::time::Duration,
) -> Result<String, CodeError> {
    use std::process::Stdio;
    use tempfile::*;
    use tokio::process;
    let mut source = NamedTempFile::new()
        .map_err(|e| CodeError::Other(format!("Cannot create temporary file: {}", e)))?;
    {
        use std::io::Write;
        let file = source.as_file_mut();
        let mut to_write = String::new();
        if let CodeLanguage::Javascript = language {
            to_write.push_str("const input=\"");
            to_write.extend(input.escape_default());
            to_write.push_str("\";\n");
        }
        to_write.push_str(code);
        file.write_all(to_write.as_ref())
            .map_err(|e| CodeError::Other(format!("Cannot write code into file: {}", e)))?;
    }
    let binary = NamedTempFile::new()
        .map_err(|e| CodeError::Other(format!("Cannot create temporary file: {}", e)))?;
    let binary_path = binary.path().to_path_buf();
    let _binary = binary.into_temp_path();
    let default_args = ["--quiet", "--overlay-tmpfs", "--private"];
    match language {
        CodeLanguage::Cpp => {
            let compiler = process::Command::new("g++")
                .args(&["-x", "c++"])
                .args(&["-o".as_ref(), binary_path.as_path()])
                .arg(source.path())
                .stderr(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .map_err(|e| CodeError::Other(format!("Cannot spawn compiler process: {}", e)))?
                .wait_with_output()
                .await
                .map_err(|e| CodeError::Other(format!("Error compiling source: {}", e)))?;
            if !compiler.stderr.is_empty() {
                return Err(CodeError::Compile(
                    String::from_utf8_lossy(&compiler.stderr).into(),
                ));
            }
            let runner = process::Command::new("firejail")
                .args(&default_args)
                .arg(binary_path.as_path())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| CodeError::Other(format!("Cannot spawn runner process: {}", e)))?;
            run_binary(runner, timeout, input).await
        }
        CodeLanguage::Bash => {
            let runner = process::Command::new("firejail")
                .args(&default_args)
                .arg("bash")
                .arg(source.path())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| CodeError::Other(format!("Cannot spawn runner process: {}", e)))?;
            run_binary(runner, timeout, input).await
        }
        CodeLanguage::Python => {
            let runner = process::Command::new("firejail")
                .args(&default_args)
                .arg("python3")
                .arg(source.path())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| CodeError::Other(format!("Cannot spawn runner process: {}", e)))?;
            run_binary(runner, timeout, input).await
        }
        CodeLanguage::Javascript => {
            let runner = process::Command::new("firejail")
                .args(&default_args)
                .arg("node")
                .arg(source.path())
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| CodeError::Other(format!("Cannot spawn runner process: {}", e)))?;
            run_binary_without_stdin(runner, timeout).await
        }
    }
}

async fn run_binary(
    mut runner: tokio::process::Child,
    timeout: std::time::Duration,
    input: &str,
) -> Result<String, CodeError> {
    if let Some(stdin) = runner.stdin() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(input.as_ref())
            .await
            .map_err(|e| CodeError::Other(format!("Cannot write to stdin: {}", e)))?;
    }
    run_binary_without_stdin(runner, timeout).await
}

async fn run_binary_without_stdin(
    runner: tokio::process::Child,
    timeout: std::time::Duration,
) -> Result<String, CodeError> {
    let output = async_std::future::timeout(timeout, runner.wait_with_output()).await;
    match output {
        Err(_) => Err(CodeError::Timeout),
        Ok(output) => {
            let output =
                output.map_err(|e| CodeError::Other(format!("Error running binary: {}", e)))?;
            if !output.stderr.is_empty() {
                Err(CodeError::Runtime(
                    String::from_utf8_lossy(&output.stderr).trim().into(),
                ))
            } else {
                Ok(String::from_utf8_lossy(&output.stdout).trim().into())
            }
        }
    }
}

use log::info;
use telegram_bot::*;
pub async fn on_code_message(
    message: types::Message,
    context: &BotContext<'_>,
) -> Result<(), telegram_bot::Error> {
    use types::message::*;
    if let Some(CodeMessage {
        code,
        language,
        input,
        prev_session,
    }) = parse_code_message(&message, context)
    {
        use std::time::Duration;
        let run_task = execute_code(
            &language,
            &code,
            &input,
            Duration::from_secs(context.config.code_timeout as u64),
        )
        .await;
        let reply = match run_task {
            Ok(output) if output.is_empty() => {
                context.api.send(message.text_reply("No output.")).await?
            }
            Ok(output) => {
                context
                    .api
                    .send(
                        message
                            .text_reply(format!("`{}`", output))
                            .parse_mode(ParseMode::Markdown),
                    )
                    .await?
            }
            Err(e) => match e {
                CodeError::Compile(e) => {
                    let mut reply_task =
                        message.text_reply(format!("<b>Compile Error</b>\n<pre>{}</pre>", e));
                    context
                        .api
                        .send(reply_task.parse_mode(ParseMode::Html))
                        .await
                }
                CodeError::Runtime(e) => {
                    let mut reply_task = message.text_reply(format!("<b>Runtime Error</b>\n{}", e));
                    context
                        .api
                        .send(reply_task.parse_mode(ParseMode::Html))
                        .await
                }
                CodeError::Other(e) => {
                    let mut reply_task =
                        message.text_reply(format!("<b>Environmental Error</b>\n{}", e));
                    context
                        .api
                        .send(reply_task.parse_mode(ParseMode::Html))
                        .await
                }
                CodeError::Timeout => {
                    let mut reply_task = message.text_reply("_Timed out._".to_string());
                    context
                        .api
                        .send(reply_task.parse_mode(ParseMode::Markdown))
                        .await
                }
            }?,
        };
        if let Ok(mut session) = context.session.write() {
            let new_session = match prev_session {
                Some(id) => CodeSession::Reference { id },
                None => CodeSession::Real { code, language },
            };
            session.put(reply.chat.id(), reply.id, Session::Code(new_session));
        }
    }
    Ok(())
}

struct CodeMessage {
    code: String,
    language: CodeLanguage,
    input: String,
    prev_session: Option<MessageId>,
}

fn parse_code_message(message: &types::Message, context: &BotContext<'_>) -> Option<CodeMessage> {
    if let types::MessageKind::Text { ref data, .. } = message.kind {
        if let Some(reply_to_message) = &message.reply_to_message {
            if let MessageOrChannelPost::Message(reply_to) = &**reply_to_message {
                let session = context
                    .session
                    .read()
                    .ok()
                    .and_then(|session| session.get(reply_to.chat.id(), reply_to.id).cloned());
                let code_session = match session {
                    Some(Session::Code(CodeSession::Reference { id })) => context
                        .session
                        .read()
                        .ok()
                        .and_then(|session| session.get(reply_to.chat.id(), id).cloned())
                        .map(|Session::Code(s)| (s, id)),
                    Some(Session::Code(s)) => Some((s, reply_to.id)),
                    _ => None,
                };
                if let Some((CodeSession::Real { code, language }, real_id)) = code_session {
                    Some(CodeMessage {
                        code,
                        language,
                        input: data.to_owned(),
                        prev_session: Some(real_id),
                    })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            let mut language = None;
            if data.starts_with("/cpp") {
                language = Some(CodeLanguage::Cpp);
                info!("/cpp from {} @ {}", message.from.id, message.chat.id());
            } else if data.starts_with("/bash") {
                language = Some(CodeLanguage::Bash);
                info!("/bash from {} @ {}", message.from.id, message.chat.id());
            } else if data.starts_with("/py") {
                language = Some(CodeLanguage::Python);
                info!("/py from {} @ {}", message.from.id, message.chat.id());
            } else if data.starts_with("/js") {
                language = Some(CodeLanguage::Javascript);
                info!("/js from {} @ {}", message.from.id, message.chat.id());
            }

            let code = if language.is_some() {
                let mut args = data.splitn(2, char::is_whitespace).skip(1);
                args.next()
            } else {
                None
            };

            if let (Some(code), Some(language)) = (code, language) {
                Some(CodeMessage {
                    code: code.to_string(),
                    language,
                    input: String::new(),
                    prev_session: None,
                })
            } else {
                None
            }
        }
    } else {
        None
    }
}
