pub mod code;

use std::sync::RwLock;
use telegram_bot::types;

#[derive(Clone)]
pub enum Session {
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
    code_api: String,
}

impl Default for BotConfig {
    fn default() -> Self {
        use std::env::var;
        let code_api = var("CODE_API").expect("CODE_API not set");
        Self { code_api }
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

    pub fn get_session(&self, chat: types::ChatId, message: types::MessageId) -> Option<Session> {
        self.session.read().ok().and_then(|session| session.get(chat, message).cloned())
    }
}
