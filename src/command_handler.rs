use std::{path::PathBuf, env};
use anyhow::Result;
use serenity::{prelude::GatewayIntents, Client};
use structopt::StructOpt;
use tracing::{error, info};
use async_openai::Client as OpenAIClient;

use crate::{conversation::ConversationCache, msg_handler::Handler, knowledge_base::{upsert_knowledge, query}};

#[derive(StructOpt, Debug)]
#[structopt(
    name = "discord-ai-bot",
    about = "A tool to upsert knowledge into a knowledge base"
)]
pub enum Opt {
    /// Start discord ai bot service
    Start,

    /// Upsert knowledge into a knowledge base
    Update {
        /// Collection name
        collection: String,

        /// JSON file to update knowledge base
        #[structopt(name = "FILE", parse(from_os_str))]
        file: PathBuf,
    },

    /// Query knowledge base
    Query {
        /// Collection name
        collection: String,

        /// A question
        question: String,
    }
}

pub async fn execute() -> Result<()> {
    let opt = Opt::from_args();
    match opt {
        Opt::Start => {
            // Configure the client with your Discord bot token in the environment.
            let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
            // Set gateway intents, which decides what events the bot will be notified about
            let intents = GatewayIntents::GUILD_MESSAGES
                | GatewayIntents::DIRECT_MESSAGES
                | GatewayIntents::MESSAGE_CONTENT;

            let openai_client = OpenAIClient::new();
            let conversation_cache = ConversationCache::default();
            let mut client = Client::builder(&token, intents)
                .event_handler(Handler {
                    openai_client,
                    conversation_cache,
                })
                .await
                .expect("Err creating discord bot client");

            if let Err(why) = client.start().await {
                error!("Client error: {:?}", why);
            }
        }
        Opt::Update { collection, file } => {
            info!("Upserting knowledge into a knowledge base: {:?}", file);
            upsert_knowledge(file, &collection).await?;
        }
        Opt::Query {collection, question} => {
            info!("Querying related fact from {:?}: {:?}", collection, question);
            query(&question, &collection).await?;
        }
    }
    Ok(())
}
