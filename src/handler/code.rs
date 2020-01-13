use crate::handler::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy)]
pub enum CodeLanguage {
    Rust,
    Cpp,
    Python,
    Javascript,
    Haskell,
    Aheui,
}

impl CodeLanguage {
    fn into_identifier(self) -> &'static str {
        match self {
            CodeLanguage::Rust => "rust",
            CodeLanguage::Cpp => "cpp",
            CodeLanguage::Python => "python",
            CodeLanguage::Javascript => "javascript",
            CodeLanguage::Haskell => "haskell",
            CodeLanguage::Aheui => "aheui",
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
enum CodeError {
    Compile { message: String },
    Runtime { message: String },
    Timeout,
    Other { message: String },
}

type CodeResult = Result<String, CodeError>;

#[derive(Serialize)]
struct CodeRequest {
    code: String,
    input: String,
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
    Replied {
        reply_id: types::MessageId,
    },
}

use log::{error, info};
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
        root_session,
        ..
    }) = parse_code_message(&message, context)
    {
        use surf::get;
        let request_body = CodeRequest {
            code: code.clone(),
            input,
        };
        let uri = format!("{}/{}", context.config.code_api, language.into_identifier());
        let request = get(uri).body_json(&request_body);
        if let Err(e) = request {
            error!("Error deserializing code request body: {}", e);
            return Ok(());
        }
        let request = request.unwrap().recv_json::<CodeResult>().await;
        if let Err(e) = request {
            error!("Error sending code request: {}", e);
            return Ok(());
        }
        let reply = match request.unwrap() {
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
                CodeError::Compile { message: e } => {
                    let mut reply_task =
                        message.text_reply(format!("<b>Compile Error</b>\n<pre>{}</pre>", e));
                    context
                        .api
                        .send(reply_task.parse_mode(ParseMode::Html))
                        .await
                }
                CodeError::Runtime { message: e } => {
                    let mut reply_task = message.text_reply(format!(
                        "<b>Runtime Error</b>\n{}",
                        e.replace("<module>", "module")
                    ));
                    context
                        .api
                        .send(reply_task.parse_mode(ParseMode::Html))
                        .await
                }
                CodeError::Other { message: e } => {
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
            let new_session = match root_session {
                Some(id) => CodeSession::Reference { id },
                None => CodeSession::Real { code, language },
            };
            session.put(reply.chat.id(), reply.id, Session::Code(new_session));
            session.put(
                message.chat.id(),
                message.id,
                Session::Code(CodeSession::Replied { reply_id: reply.id }),
            );
        }
    }
    Ok(())
}

pub async fn on_code_update(
    message: types::Message,
    context: &BotContext<'_>,
) -> Result<(), telegram_bot::Error> {
    use types::message::*;
    if let Some(CodeMessage {
        code,
        language,
        input,
        root_session,
        prev_session,
    }) = parse_code_message(&message, context)
    {
        if let Some(prev_session) = prev_session {
            use surf::get;
            let request_body = CodeRequest {
                code: code.clone(),
                input,
            };
            let uri = format!("{}/{}", context.config.code_api, language.into_identifier());
            let request = get(uri).body_json(&request_body);
            if let Err(e) = request {
                error!("Error deserializing code request body: {}", e);
                return Ok(());
            }
            let request = request.unwrap().recv_json::<CodeResult>().await;
            if let Err(e) = request {
                error!("Error sending code request: {}", e);
                return Ok(());
            }
            let reply = match request.unwrap() {
                Ok(output) if output.is_empty() => {
                    context
                        .api
                        .send(EditMessageText::new(
                            message.chat,
                            prev_session,
                            "No output.",
                        ))
                        .await?
                }
                Ok(output) => {
                    context
                        .api
                        .send(
                            EditMessageText::new(
                                message.chat,
                                prev_session,
                                format!("`{}`", output),
                            )
                            .parse_mode(ParseMode::Markdown),
                        )
                        .await?
                }
                Err(e) => match e {
                    CodeError::Compile { message: e } => {
                        let mut reply_task = EditMessageText::new(
                            message.chat,
                            prev_session,
                            format!("<b>Compile Error</b>\n<pre>{}</pre>", e),
                        );
                        context
                            .api
                            .send(reply_task.parse_mode(ParseMode::Html))
                            .await
                    }
                    CodeError::Runtime { message: e } => {
                        let mut reply_task = EditMessageText::new(
                            message.chat,
                            prev_session,
                            format!("<b>Runtime Error</b>\n{}", e),
                        );
                        context
                            .api
                            .send(reply_task.parse_mode(ParseMode::Html))
                            .await
                    }
                    CodeError::Other { message: e } => {
                        let mut reply_task = EditMessageText::new(
                            message.chat,
                            prev_session,
                            format!("<b>Environmental Error</b>\n{}", e),
                        );
                        context
                            .api
                            .send(reply_task.parse_mode(ParseMode::Html))
                            .await
                    }
                    CodeError::Timeout => {
                        let mut reply_task = EditMessageText::new(
                            message.chat,
                            prev_session,
                            "_Timed out._".to_string(),
                        );
                        context
                            .api
                            .send(reply_task.parse_mode(ParseMode::Markdown))
                            .await
                    }
                }?,
            };
            if let Ok(mut session) = context.session.write() {
                let new_session = match root_session {
                    Some(id) => CodeSession::Reference { id },
                    None => CodeSession::Real { code, language },
                };
                session.put(reply.chat.id(), reply.id, Session::Code(new_session));
            }
            Ok(())
        } else {
            on_code_message(message, context).await
        }
    } else {
        Ok(())
    }
}

struct CodeMessage {
    code: String,
    language: CodeLanguage,
    input: String,
    root_session: Option<MessageId>,
    prev_session: Option<MessageId>,
}

fn parse_code_message(message: &types::Message, context: &BotContext<'_>) -> Option<CodeMessage> {
    if let types::MessageKind::Text { ref data, .. } = message.kind {
        let prev_session = context
            .get_session(message.chat.id(), message.id)
            .and_then(|session| match session {
                Session::Code(CodeSession::Replied { reply_id }) => Some(reply_id),
                _ => None,
            });
        if let Some(reply_to_message) = &message.reply_to_message {
            if let MessageOrChannelPost::Message(reply_to) = &**reply_to_message {
                let session = context.get_session(reply_to.chat.id(), reply_to.id);
                let code_session = match session {
                    Some(Session::Code(CodeSession::Reference { id })) => context
                        .get_session(reply_to.chat.id(), id)
                        .map(|Session::Code(s)| (s, id)),
                    Some(Session::Code(s)) => Some((s, reply_to.id)),
                    _ => None,
                };
                if let Some((CodeSession::Real { code, language }, real_id)) = code_session {
                    Some(CodeMessage {
                        code,
                        language,
                        input: data.to_owned(),
                        root_session: Some(real_id),
                        prev_session,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            let mut language = None;
            if data.starts_with("/rust") {
                language = Some(CodeLanguage::Rust);
                info!("/rust from {} @ {}", message.from.id, message.chat.id());
            } else if data.starts_with("/cpp") {
                language = Some(CodeLanguage::Cpp);
                info!("/cpp from {} @ {}", message.from.id, message.chat.id());
            } else if data.starts_with("/py") {
                language = Some(CodeLanguage::Python);
                info!("/py from {} @ {}", message.from.id, message.chat.id());
            } else if data.starts_with("/js") {
                language = Some(CodeLanguage::Javascript);
                info!("/js from {} @ {}", message.from.id, message.chat.id());
            } else if data.starts_with("/hs") {
                language = Some(CodeLanguage::Haskell);
                info!("/hs from {} @ {}", message.from.id, message.chat.id());
            } else if data.starts_with("/ah") {
                language = Some(CodeLanguage::Aheui);
                info!("/ah from {} @ {}", message.from.id, message.chat.id());
            }

            let code = if language.is_some() {
                let mut args = data.splitn(2, char::is_whitespace).skip(1);
                Some(args.next().unwrap_or(""))
            } else {
                None
            };

            if let (Some(code), Some(language)) = (code, language) {
                Some(CodeMessage {
                    code: code.to_string(),
                    language,
                    input: String::new(),
                    root_session: None,
                    prev_session,
                })
            } else {
                None
            }
        }
    } else {
        None
    }
}
