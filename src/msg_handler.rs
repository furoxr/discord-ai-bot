use tracing::{error, info, trace};

use crate::helper::try_log;

use async_openai::types::{
    ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role,
};
use async_openai::Client as OpenAIClient;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

pub struct Handler {
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
                let request_build = CreateChatCompletionRequestArgs::default()
                    .model("gpt-3.5-turbo")
                    .messages([
                        try_log!(ChatCompletionRequestMessageArgs::default()
                            .role(Role::System)
                            .content("You are a helpful assistant.")
                            .build()),
                        try_log!(ChatCompletionRequestMessageArgs::default()
                            .role(Role::User)
                            .content(real_content)
                            .build()),
                    ])
                    .build();
                let request = try_log!(request_build);
                let response = try_log!(self.openai_client.chat().create(request).await);

                for choice in response.choices {
                    trace!("{}", &choice.message.content);
                    if let Err(why) = msg.channel_id.send_message(
                        &ctx.http,
                         |m| {
                            m.content(choice.message.content)
                             .reference_message(&msg)
                        }).await {
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
