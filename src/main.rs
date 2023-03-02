use std::env;
use tracing::{info, error, debug, trace};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt};

use async_openai::types::CreateCompletionRequestArgs;
use async_openai::Client as OpenAIClient;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

struct Handler {
    pub openai_client: OpenAIClient,
}

#[async_trait]
impl EventHandler for Handler {
    // Set a handler for the `message` event - so that whenever a new message
    // is received - the closure (or function) passed will be called.
    //
    // Event handlers are dispatched through a threadpool, and so multiple
    // events can be dispatched simultaneously.
    async fn message(&self, ctx: Context, msg: Message) {
        match msg.mentions_me(&ctx).await {
            Err(why) => {
                error!("Error check mentions_me: {:?}", why);
            }
            Ok(true) => {
                info!(
                    "Mentioned by {:?}, Content: {:?}",
                    &msg.author.name, &msg.content
                );

                let mention_part =
                    String::from("<@") + &ctx.cache.current_user_id().0.to_string() + ">";
                if !msg.content.starts_with(&mention_part) {
                    return;
                }
                let index = msg.content.find('>').unwrap_or(0);
                if index + 1 > msg.content.len() - 2 {
                    return;
                }

                let real_content = &msg.content[index + 2..];

                let request = CreateCompletionRequestArgs::default()
                    .model("text-davinci-003")
                    .prompt(real_content)
                    .max_tokens(3500_u16)
                    .n(1)
                    .build()
                    .expect("Failed to build request!");

                let response = match self.openai_client.completions().create(request).await {
                    Err(why) => {
                        error!("Error in openai completion: {:?}", why);
                        return;
                    }
                    Ok(x) => x,
                };

                for choice in response.choices {
                    trace!("{}", &choice.text);
                    if let Err(why) = msg.channel_id.say(&ctx.http, choice.text).await {
                        error!("Error sending message: {:?}", why);
                    }
                }

                if real_content == "!ping" {
                    if let Err(why) = msg.channel_id.say(&ctx.http, "Pong!").await {
                        error!("Error sending message: {:?}", why);
                    }
                }
            }
            Ok(false) => {
                info!("Content: {:?}", &msg.content);
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

fn init_tracing() -> WorkerGuard {
    let file_appender = tracing_appender::rolling::hourly("./logs", "log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    tracing::subscriber::set_global_default(
        fmt::Subscriber::builder()
            // subscriber configuration
            .with_env_filter(EnvFilter::from_default_env())
            .finish()
            // add additional writers
            .with(fmt::Layer::default().with_writer(file_writer))
    ).expect("Unable to set global tracing subscriber");
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
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler { openai_client })
        .await
        .expect("Err creating discord bot client");

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}
