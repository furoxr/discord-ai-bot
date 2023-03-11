use anyhow::Result;
use serenity::{prelude::GatewayIntents, Client};
use std::path::PathBuf;
use structopt::StructOpt;
use tracing::{error, info};

use crate::{
    conversation::ConversationCache,
    knowledge_base::{clear_collection, query, upsert_knowledge, KnowledgeClient},
    msg_handler::Handler, ai::Openai,
};

#[derive(StructOpt, Debug)]
#[structopt(
    name = "discord-ai-bot",
    about = "A tool that enables the creation of a Discord AI bot service utilizing the power of GPT-3.5"
)]
pub struct DiscordAiBot {
    /// Openai api key
    #[structopt(name = "openai-api-key", env = "OPENAI_API_KEY")]
    openai_api_key: String,

    #[structopt(
        name = "qdrant-rpc-url",
        default_value = "http://localhost:6334",
        help = "Qdrant database grpc query url"
    )]
    qdrant_grpc_url: String,

    #[structopt(subcommand)]
    cmd: Opt,
}

#[derive(StructOpt, Debug)]
#[structopt(name = "discord-ai-bot")]
pub enum Opt {
    /// Start discord ai bot service
    Start {
        /// Discord bot token
        #[structopt(name = "discord-bot-token", env = "DISCORD_TOKEN")]
        discord_bot_token: String,
    },

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
    },

    /// Clear collection
    Clear {
        /// Collection name
        collection: String,
    },
}

pub async fn execute() -> Result<()> {
    let DiscordAiBot {
        qdrant_grpc_url,
        openai_api_key,
        cmd,
    } = DiscordAiBot::from_args();

    match cmd {
        Opt::Start { discord_bot_token } => {
            // Set gateway intents, which decides what events the bot will be notified about
            let intents = GatewayIntents::GUILD_MESSAGES
                | GatewayIntents::DIRECT_MESSAGES
                | GatewayIntents::MESSAGE_CONTENT;

            let openai_client = Openai::new(&openai_api_key)?;
            let conversation_cache = ConversationCache::default();
            let qdrant_client = KnowledgeClient::new(&qdrant_grpc_url).await?;
            let mut client = Client::builder(&discord_bot_token, intents)
                .event_handler(Handler {
                    openai_client,
                    conversation_cache,
                    knowledge_client: qdrant_client,
                })
                .await
                .expect("Err creating discord bot client");

            if let Err(why) = client.start().await {
                error!("Client error: {:?}", why);
            }
        }
        Opt::Update { collection, file } => {
            info!("Upserting knowledge into a knowledge base: {:?}", file);
            upsert_knowledge(&qdrant_grpc_url, file, &collection).await?;
        }
        Opt::Query {
            collection,
            question,
        } => {
            info!(
                "Querying related fact from {:?}: {:?}",
                collection, question
            );
            query(&qdrant_grpc_url, &question, &collection).await?;
        }
        Opt::Clear { collection } => {
            info!("Clearing collection: {:?}", collection);
            clear_collection(&qdrant_grpc_url, &collection).await?;
        }
    }
    Ok(())
}
