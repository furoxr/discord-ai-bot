use anyhow::Result;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs,
    CreateChatCompletionRequestArgs, Role,
};
use async_openai::Client as OpenAIClient;
use serenity::model::prelude::UserId;
use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready},
    prelude::*,
};
use tracing::{error, info, trace};

use crate::conversation::ConversationCache;
use crate::helper::try_log;

pub struct Handler {
    pub openai_client: OpenAIClient,
    pub conversation_cache: ConversationCache,
}

impl Handler {
    fn extract_legal_content(bot_user_id: UserId, msg: &Message) -> Option<&str> {
        let mention_part = String::from("<@") + &bot_user_id.0.to_string() + ">";
        if !msg.content.starts_with(&mention_part) {
            return None;
        }
        let index = msg.content.find('>').unwrap_or(0);
        if index + 1 > msg.content.len() - 2 {
            return None;
        }
        let real_content = &msg.content[index + 2..];
        Some(real_content)
    }

    fn build_conversation(
        &self,
        question: &str,
        user_id: UserId,
    ) -> Result<Vec<ChatCompletionRequestMessage>> {
        let mut conversations = vec![ChatCompletionRequestMessageArgs::default()
            .role(Role::System)
            .content("You are a helpful assistant.")
            .build()?];

        let history = self
            .conversation_cache
            .get_messages(user_id)?
            .into_iter()
            .map(|m| m.try_into())
            .collect::<Result<Vec<ChatCompletionRequestMessage>, _>>()?;
        conversations.extend(history);
        conversations.push(
            ChatCompletionRequestMessageArgs::default()
                .role(Role::User)
                .content(question)
                .build()?,
        );

        Ok(conversations)
    }

    async fn get_ai_response(&self, question: &str, user_id: UserId) -> Result<String> {
        let conversations = self.build_conversation(question, user_id)?;
        let request = CreateChatCompletionRequestArgs::default()
            .model("gpt-3.5-turbo")
            .messages(conversations)
            .build()?;
        let mut response = self.openai_client.chat().create(request).await?;
        if let Some(choice) = response.choices.pop() {
            trace!("{}", &choice.message.content);
            Ok(choice.message.content)
        } else {
            Err(anyhow::anyhow!("No response from OpenAI"))
        }
    }
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
            Ok(false) => {
                trace!("Content: {:?}", &msg.content);
            }
            Ok(true) => {
                info!(
                    "Mentioned by {:?}, Content: {:?}",
                    &msg.author.name, &msg.content
                );

                let real_content =
                    match Self::extract_legal_content(ctx.cache.current_user_id(), &msg) {
                        Some(x) => x,
                        None => return,
                    };
                let response = try_log!(self.get_ai_response(real_content, msg.author.id).await);
                let response_sent = try_log!(
                    msg.channel_id
                        .send_message(&ctx.http, |m| {
                            m.content(response).reference_message(&msg)
                        })
                        .await
                );

                vec![(Role::User, msg.clone()), (Role::Assistant, response_sent)]
                    .into_iter()
                    .for_each(|x| {
                        try_log!(self.conversation_cache.add_message(msg.author.id, x.0, x.1))
                    });
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}
