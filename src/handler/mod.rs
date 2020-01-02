pub mod code;

use std::sync::RwLock;
use telegram_bot::types;

#[derive(Clone)]
enum Session {
    Code(code::CodeSession),
}

#[derive(Hash, PartialEq, Eq)]
struct UniqueMessageId(types::ChatId, types::MessageId);

struct SessionStorage {
    map: std::collections::HashMap<UniqueMessageId, Session>,
}

impl SessionStorage {
    fn new() -> Self {
        Self {
            map: std::collections::HashMap::new(),
        }
    }

    fn get(&self, chat: types::ChatId, message: types::MessageId) -> Option<&Session> {
        self.map.get(&UniqueMessageId(chat, message))
    }

    fn put(&mut self, chat: types::ChatId, message: types::MessageId, value: Session) {
        self.map.insert(UniqueMessageId(chat, message), value);
    }
}

pub struct BotConfig {
    code_timeout: u8,
}

impl Default for BotConfig {
    fn default() -> Self {
        use std::env::var;
        let code_timeout = var("CODE_TIMEOUT")
            .map(|raw| raw.parse().expect("Cannot parse CODE_TIMEOUT"))
            .unwrap_or(5);
        Self { code_timeout }
    }
}

pub struct BotContext<'a> {
    api: &'a telegram_bot::Api,
    session: RwLock<SessionStorage>,
    config: BotConfig,
}

impl<'a> BotContext<'a> {
    pub fn new(api: &'a telegram_bot::Api, config: BotConfig) -> Self {
        Self {
            api,
            session: RwLock::new(SessionStorage::new()),
            config,
        }
    }
}
