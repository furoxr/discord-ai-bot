use anyhow::{anyhow, Result};
use async_openai::{
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs,
        CreateChatCompletionRequestArgs, CreateEmbeddingRequestArgs, Role,
    },
    Client as OpenAIClient,
};
use qdrant_client::qdrant::{
    value::Kind, with_payload_selector::SelectorOptions, ScoredPoint, SearchPoints,
    WithPayloadSelector,
};
use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready, prelude::UserId},
    prelude::*,
};
use tracing::{error, info, trace, debug};

use crate::{conversation::ConversationCache, helper::try_log, knowledge_base::KnowledgeClient};

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

    fn build_conversation(&self, user_id: UserId) -> Result<Vec<ChatCompletionRequestMessage>> {
        let mut conversations = vec![ChatCompletionRequestMessageArgs::default()
            .role(Role::System)
            .content("I will ask with format like this:
            Question: {text}
            Knowledge: {text}
            You are a helpful assistant, and you should answer question after the 'Question'.
            And there may be related knowledge after knowledge you could refer to. ")
            .build()?];

        let history = self
            .conversation_cache
            .get_messages(user_id)?
            .into_iter()
            .map(|m| m.try_into())
            .collect::<Result<Vec<ChatCompletionRequestMessage>, _>>()?;
        conversations.extend(history);
        Ok(conversations)
    }

    pub async fn query_knowledge(&self, embedding: Vec<f32>) -> Result<ScoredPoint> {
        let search = SearchPoints {
            collection_name: "darwinia".into(),
            vector: embedding,
            filter: None,
            limit: 3,
            with_payload: Some(WithPayloadSelector {
                selector_options: Some(SelectorOptions::Enable(true)),
            }),
            params: None,
            // score_threshold: Some(0.8),
            score_threshold: None,
            offset: None,
            vector_name: None,
            with_vectors: None,
            read_consistency: None,
        };
        let mut response = self.knowledge_client.search_points(&search).await?;
        response.result.reverse();
        if let Some(response) = response.result.pop() {
            Ok(response)
        } else {
            Err(anyhow!("No result found"))
        }
    }

    async fn get_response_with_knowledge(&self, question: &str, user_id: UserId) -> Result<String> {
        let embedding = self.get_embedding(question).await?;
        match self.query_knowledge(embedding).await {
            // If there is no knowledge, just use openai to generate response.
            Err(_) => self.get_response(question, user_id).await,

            // If there is knowledge, append it to conversation.
            Ok(knowledge) => {
                let mut conversations = self.build_conversation(user_id)?;
                let content_value = knowledge
                    .payload
                    .get("content")
                    .ok_or_else(|| anyhow!("Incorect knowledge format!"))?;
                let content = if let Some(Kind::StringValue(content)) = content_value.kind.clone() {
                    content
                } else {
                    return Err(anyhow!("Incorect knowledge format!"));
                };
                let ref_url_value = knowledge
                    .payload
                    .get("url")
                    .ok_or_else(|| anyhow!("Incorect knowledge format!"))?;
                let _ref_url = if let Some(Kind::StringValue(url)) = ref_url_value.kind.clone() {
                    url
                } else {
                    return Err(anyhow!("Incorect knowledge format!"));
                };
                debug!("Knowledge url: {}", &_ref_url);
                let context = format!("Question: {}\nKnowledge: {}", question, &content);
                conversations.push(
                    ChatCompletionRequestMessageArgs::default()
                        .role(Role::User)
                        .content(context)
                        .build()?,
                );
                self.get_chat_complete(conversations).await
                // response.push_str(&format!("\n\n ref: {}", ref_url));
            }
        }
    }

    async fn get_chat_complete(
        &self,
        conversations: Vec<ChatCompletionRequestMessage>,
    ) -> Result<String> {
        let request = CreateChatCompletionRequestArgs::default()
            .model("gpt-3.5-turbo")
            .messages(conversations)
            .build()?;
        let mut response = self.openai_client.chat().create(request).await?;
        if let Some(choice) = response.choices.pop() {
            trace!("{}", &choice.message.content);
            Ok(choice.message.content)
        } else {
            Err(anyhow!("No chat response from OpenAI"))
        }
    }

    async fn get_response(&self, question: &str, user_id: UserId) -> Result<String> {
        let mut conversations = self.build_conversation(user_id)?;
        conversations.push(
            ChatCompletionRequestMessageArgs::default()
                .role(Role::User)
                .content(question)
                .build()?,
        );
        self.get_chat_complete(conversations).await
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

                let typing = try_log!(msg.channel_id.start_typing(&ctx.http));
                let real_content =
                    match Self::extract_legal_content(ctx.cache.current_user_id(), &msg) {
                        Some(x) => x,
                        None => return,
                    };
                let response = try_log!(
                    self.get_response_with_knowledge(real_content, msg.author.id)
                        .await
                );
                let _t = typing.stop();
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
