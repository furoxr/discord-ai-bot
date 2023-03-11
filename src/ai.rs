use std::collections::VecDeque;

use anyhow::{anyhow, Result};
use async_openai::{
    types::{
        ChatCompletionRequestMessage, CreateChatCompletionRequestArgs, CreateEmbeddingRequestArgs,
    },
    Client,
};
use tiktoken_rs::tiktoken::{cl100k_base, CoreBPE};
use tracing::trace;

use crate::conversation::ConversationCtx;

pub static GPT_MODEL: &str = "gpt-3.5-turbo";
pub static EMBEDDING_MODEL: &str = "text-embedding-ada-002";
pub static CHAT_GPT_LIMIT: usize = 4096;

/// Calculate tokens consumed in the chat api of openai. Check the calculation algorithm here:
/// https://github.com/openai/openai-cookbook/blob/main/examples/How_to_count_tokens_with_tiktoken.ipynb
pub fn num_tokens_from_messages(
    messages: &VecDeque<ChatCompletionRequestMessage>,
    model: &str,
) -> Result<usize> {
    let bpe = cl100k_base()?;
    let mut num_tokens = 0;
    if model == "gpt-3.5-turbo-0301" {
        for msg in messages.iter() {
            num_tokens += 4;
            num_tokens += bpe.encode_with_special_tokens(&msg.content).len();
            num_tokens += bpe.encode_with_special_tokens(&msg.role.to_string()).len();
            if let Some(name) = &msg.name {
                num_tokens += bpe.encode_with_special_tokens(name).len() - 1;
            }
        }
        num_tokens += 2;
    }

    Ok(num_tokens)
}

pub struct TokenEncoder(pub CoreBPE);

impl TokenEncoder {
    pub fn new() -> Result<Self> {
        Ok(Self(cl100k_base()?))
    }

    pub fn num_tokens_from_message(&self, message: &ChatCompletionRequestMessage) -> Result<usize> {
        let mut num_tokens = 0;
        num_tokens += self.0.encode_with_special_tokens(&message.content).len();
        num_tokens += self
            .0
            .encode_with_special_tokens(&message.role.to_string())
            .len();
        if let Some(name) = &message.name {
            num_tokens += self.0.encode_with_special_tokens(name).len() - 1;
        }

        Ok(num_tokens)
    }

    pub fn num_tokens_from_messages(
        &self,
        messages: &VecDeque<ChatCompletionRequestMessage>,
    ) -> Result<usize> {
        let mut num_tokens = 0;
        for msg in messages.iter() {
            num_tokens += 4;
            num_tokens += self.num_tokens_from_message(msg)?;
        }
        num_tokens += 2;

        Ok(num_tokens)
    }
}

pub struct Openai(pub Client, pub TokenEncoder);

impl Openai {
    pub fn new(api_key: &str) -> Result<Self> {
        Ok(Self(
            Client::new().with_api_key(api_key),
            TokenEncoder::new()?,
        ))
    }

    pub fn shrink_conversation<'a>(
        &'a self,
        ctx: &'a mut ConversationCtx,
        limit: usize,
    ) -> Result<&mut ConversationCtx> {
        let mut messages_count = VecDeque::with_capacity(ctx.value.len());
        let mut tokens: usize = 0;
        for msg in ctx.value.iter() {
            tokens += 4;
            let num_tokens = self.1.num_tokens_from_message(msg)?;
            tokens += num_tokens;
            messages_count.push_back(num_tokens);
        }
        tokens += 2;

        if tokens <= limit {
            return Ok(ctx);
        } else if tokens > limit && messages_count.len() <= 2 {
            return Err(anyhow!("Conversation is too short to shrink."));
        } else {
            messages_count.pop_front();
            let mut i = 0;
            while tokens > limit && messages_count.len() > 1 {
                tokens -= messages_count[0];
                tokens -= 4;
                messages_count.pop_front();
                i += 1;
            }

            if tokens > limit {
                return Err(anyhow!("Conversation can not be shrinked. "));
            }
            ctx.value.drain(1..=i);
        }

        Ok(ctx)
    }
}

impl Openai {
    pub async fn chat_complete(&self, conversation: ConversationCtx) -> Result<String> {
        let request = CreateChatCompletionRequestArgs::default()
            .model(GPT_MODEL)
            .messages(conversation.value)
            .build()?;
        let mut response = self.0.chat().create(request).await?;
        if let Some(choice) = response.choices.pop() {
            trace!("{}", &choice.message.content);
            Ok(choice.message.content)
        } else {
            Err(anyhow!("No chat response from OpenAI"))
        }
    }

    pub async fn embedding(&self, text: &str) -> Result<Vec<f32>> {
        trace!("Get embedding for '{}'", text);
        let request = CreateEmbeddingRequestArgs::default()
            .model(EMBEDDING_MODEL)
            .input(text)
            .build()?;

        let mut response = self.0.embeddings().create(request).await?;

        if let Some(data) = response.data.pop() {
            Ok(data.embedding)
        } else {
            Err(anyhow!("No embedding response from OpenAI"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Openai;
    use crate::conversation::ConversationCtx;

    fn data() -> ConversationCtx {
        let mut ctx = ConversationCtx::default();
        ctx.add_system_message("You are a helpful, pattern-following assistant that translates corporate jargon into plain English.", None)
        .add_system_message("New synergies will help drive top-line growth.", Some("example_user".into()))
        .add_system_message("Things working well together will increase revenue.", Some("example_assistant".into()))
        .add_system_message("Let's circle back when we have more bandwidth to touch base on opportunities for increased leverage.", Some("example_user".into()))
        .add_system_message("Let's talk later when we're less busy about how to do better.", Some("example_assistant".into()))
        .add_user_message("This late pivot means we don't have time to boil the ocean for the client deliverable.", None);
        ctx
    }

    #[test]
    fn test_token_calculation() {
        let ctx = data();
        let ai = Openai::new("test").unwrap();
        let nums = ai.1.num_tokens_from_messages(&ctx.value).unwrap();
        assert_eq!(nums, 126);
    }

    #[test]
    fn test_shrink_conversation() {
        let ai = Openai::new("test").unwrap();
        let mut ctx = data();
        let result = ai.shrink_conversation(&mut ctx, 125);
        assert!(result.is_ok());

        let mut ctx = data();
        let result = ai.shrink_conversation(&mut ctx, 49);
        assert!(result.is_ok());
        assert!(result.unwrap().value.len() == 2);
        assert_eq!(ctx.value[1].content, "This late pivot means we don't have time to boil the ocean for the client deliverable.");

        let mut ctx = data();
        let result = ai.shrink_conversation(&mut ctx, 48);
        assert!(result.is_err());

        let mut ctx = data();
        let result = ai.shrink_conversation(&mut ctx, 71);
        assert!(result.is_ok());
        assert!(result.unwrap().value.len() == 3);
        assert_eq!(
            ctx.value[1].content,
            "Let's talk later when we're less busy about how to do better."
        );
    }
}
