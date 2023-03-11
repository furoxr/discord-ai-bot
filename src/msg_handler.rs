use std::collections::VecDeque;

use anyhow::{anyhow, Result};
use async_openai::{
    types::{
        ChatCompletionRequestMessage, CreateChatCompletionRequestArgs, CreateEmbeddingRequestArgs,
        Role,
    },
    Client as OpenAIClient,
};
use log_error::LogError;
use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready, prelude::UserId},
    prelude::*,
};
use tracing::{debug, error, info, trace};

use crate::{
    conversation::{ConversationCache, ConversationCtx},
    helper::try_log,
    knowledge_base::{KnowledgeClient, KnowledgePayload},
};

pub struct Handler {
    pub openai_client: OpenAIClient,
    pub conversation_cache: ConversationCache,
    pub knowledge_client: KnowledgeClient,
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

    fn build_conversation(&self, user_id: UserId) -> Result<ConversationCtx> {
        let mut conversation = ConversationCtx::default();
        conversation.add_system_message(
            "I will ask with format like this:
        Question: {text}
        Knowledge: {text}
        You are a helpful assistant, and you should answer question after the 'Question'.
        And there may be related knowledge after knowledge you could refer to. ",
        None
        );

        let history: VecDeque<ChatCompletionRequestMessage> =
            self.conversation_cache.get_messages(user_id)?.into();
        conversation.extend(history);
        Ok(conversation)
    }

    pub async fn query_knowledge(&self, embedding: Vec<f32>) -> Result<KnowledgePayload> {
        let mut response = self
            .knowledge_client
            .query_knowledge("darwinia", embedding, Some(0.78))
            .await?;
        response.reverse();

        if let Some(response) = response.pop() {
            Ok(response)
        } else {
            Err(anyhow!("No result found"))
        }
    }

    fn build_conversation_with_knowledge(
        &self,
        mut conversation: ConversationCtx,
        knowledge: KnowledgePayload,
        question: &str,
    ) -> Result<ConversationCtx> {
        debug!("Knowledge url: {}", &knowledge.url);
        let context = format!("Question: {}\nKnowledge: {}", question, &knowledge.content);
        conversation.add_user_message(&context, None);
        Ok(conversation)
    }

    async fn get_chat_complete(&self, conversation: ConversationCtx) -> Result<String> {
        let request = CreateChatCompletionRequestArgs::default()
            .model("gpt-3.5-turbo")
            .messages(conversation.value)
            .build()?;
        let mut response = self.openai_client.chat().create(request).await?;
        if let Some(choice) = response.choices.pop() {
            trace!("{}", &choice.message.content);
            Ok(choice.message.content)
        } else {
            Err(anyhow!("No chat response from OpenAI"))
        }
    }

    async fn get_embedding(&self, question: &str) -> Result<Vec<f32>> {
        debug!("Get embedding for '{}'", question);
        let request = CreateEmbeddingRequestArgs::default()
            .model("text-embedding-ada-002")
            .input(question)
            .build()?;

        let mut response = self.openai_client.embeddings().create(request).await?;

        if let Some(data) = response.data.pop() {
            debug!(
                "[{}] has embedding of length {}",
                data.index,
                data.embedding.len()
            );
            Ok(data.embedding)
        } else {
            Err(anyhow!("No embedding response from OpenAI"))
        }
    }

    async fn _message(&self, ctx: Context, msg: Message) -> Result<()> {
        match msg.mentions_me(&ctx).await {
            Err(why) => {
                error!("Error check mentions_me: {:?}", why);
                Ok(())
            }
            Ok(false) => {
                trace!("Content: {:?}", &msg.content);
                Ok(())
            }
            Ok(true) => {
                info!(
                    "Mentioned by {:?}, Content: {:?}",
                    &msg.author.name, &msg.content
                );

                let typing = msg.channel_id.start_typing(&ctx.http)?;
                let real_content =
                    match Self::extract_legal_content(ctx.cache.current_user_id(), &msg) {
                        Some(x) => x,
                        None => return Ok(()),
                    };

                let mut conversation = self.build_conversation(msg.author.id)?;
                let embedding = self.get_embedding(real_content).await?;
                let conversation = match self.query_knowledge(embedding).await {
                    Ok(knowledge) => self.build_conversation_with_knowledge(
                        conversation,
                        knowledge,
                        real_content,
                    )?,
                    Err(_) => {
                        conversation.add_user_message(real_content, None);
                        conversation
                    }
                };

                let response = self.get_chat_complete(conversation).await?;
                let _t = typing.stop();
                let response_sent = msg
                    .channel_id
                    .send_message(&ctx.http, |m| m.content(response).reference_message(&msg))
                    .await?;

                vec![(Role::User, msg.clone()), (Role::Assistant, response_sent)]
                    .into_iter()
                    .for_each(|x| {
                        self.conversation_cache
                            .add_message(msg.author.id, x.0, &x.1.content, None)
                            .log_error("Cache Conversation failed");
                    });
                Ok(())
            }
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
        try_log!(self._message(ctx, msg).await)
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}
