use std::env;

use async_std::prelude::*;
use log::error;
use telegram_bot::types;
use telegram_bot::*;

mod handler;
use handler::*;

#[tokio::main]
async fn main() {
    setup_logger().expect("Cannot initialize logger");

    let token = env::var("BOT_TOKEN").expect("BOT_TOKEN not set");
    let api = Api::new(token);

    let mut stream = api.stream();
    let context = handler::BotContext::new(&api, BotConfig::default());
    while let Some(update) = stream.next().await {
        match update {
            Ok(update) => {
                if let Err(e) = on_update(update, &context).await {
                    error!("Error processing update: {}", e);
                }
            }
            Err(e) => {
                error!("Error on update: {}", e);
            }
        }
    }
}

fn setup_logger() -> Result<(), log4rs::Error> {
    log4rs::init_file("log4rs.yml", Default::default())
}

async fn on_update(
    update: telegram_bot::update::Update,
    context: &BotContext<'_>,
) -> Result<(), telegram_bot::Error> {
    use types::update::*;
    if let UpdateKind::Message(message) = update.kind {
        code::on_code_message(message, context).await?;
    } else if let UpdateKind::EditedMessage(message) = update.kind {
        code::on_code_update(message, context).await?;
    }
    Ok(())
}
