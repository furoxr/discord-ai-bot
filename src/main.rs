pub mod helper;
pub mod msg_handler;
pub mod conversation;

use conversation::ConversationCache;
use msg_handler::Handler;

use std::env;
use tracing::{debug, error};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

use async_openai::Client as OpenAIClient;
use serenity::prelude::*;


fn init_tracing() -> WorkerGuard {
    let file_appender = tracing_appender::rolling::hourly("./logs", "log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    tracing::subscriber::set_global_default(
        fmt::Subscriber::builder()
            // subscriber configuration
            .with_env_filter(EnvFilter::from_default_env())
            .finish()
            // add additional writers
            .with(fmt::Layer::default().with_writer(file_writer)),
    )
    .expect("Unable to set global tracing subscriber");
    debug!("Tracing initialized.");
    guard
}

#[tokio::main]
async fn main() {
    let _guard = init_tracing();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let openai_client = OpenAIClient::new();
    let conversation_cache = ConversationCache::default();
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler { openai_client, conversation_cache })
        .await
        .expect("Err creating discord bot client");

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}
