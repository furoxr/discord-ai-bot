use std::env;

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
                println!("Error check mentions_me: {:?}", why);
            }
            Ok(true) => {
                println!(
                    "Mentioned by {:?}, Content: {:?}",
                    &msg.author, &msg.content
                );

                let mention_part =
                    String::from("<@") + &ctx.cache.current_user_id().0.to_string() + ">";
                if !msg.content.starts_with(&mention_part) {
                    return ();
                }
                let index = msg.content.find(">").unwrap_or(0);
                if index + 1 > msg.content.len() - 2 {
                    return ();
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
                        println!("Error in openai completion: {:?}", why);
                        return ();
                    },
                    Ok(x) => x,
                };

                for choice in response.choices {
                    println!("{}", &choice.text);
                    if let Err(why) = msg.channel_id.say(&ctx.http, choice.text).await {
                        println!("Error sending message: {:?}", why);
                    }
                }

                if real_content == "!ping" {
                    if let Err(why) = msg.channel_id.say(&ctx.http, "Pong!").await {
                        println!("Error sending message: {:?}", why);
                    }
                }
            }
            Ok(false) => {
                println!("Content: {:?}", &msg.content);
            }
        }
    }

    // Set a handler to be called on the `ready` event. This is called when a
    // shard is booted, and a READY payload is sent by Discord. This payload
    // contains data like the current user's guild Ids, current user data,
    // private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    // Create a new instance of the Client, logging in as a bot. This will
    // automatically prepend your bot token with "Bot ", which is a requirement
    // by Discord for bot users.
    let openai_client = OpenAIClient::new();
    let request = CreateCompletionRequestArgs::default()
        .model("text-davinci-003")
        .prompt("Who are you?")
        .max_tokens(3500_u16)
        .n(1)
        .build()
        .unwrap();

    let response = openai_client.completions().create(request).await.unwrap();

    for choice in response.choices {
        println!("{}", &choice.text);
    }

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler { openai_client })
        .await
        .expect("Err creating client");

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
