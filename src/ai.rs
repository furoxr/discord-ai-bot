use anyhow::Result;
use tiktoken_rs::tiktoken::cl100k_base;

use crate::conversation::ConversationCtx;

/// Calculate tokens consumed in the chat api of openai. Check the calculation algorithm here:
/// https://github.com/openai/openai-cookbook/blob/main/examples/How_to_count_tokens_with_tiktoken.ipynb
fn num_tokens_from_messages(messages: &ConversationCtx, model: &str) -> Result<usize> {
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

    return Ok(num_tokens);
}

// test mod
#[cfg(test)]
mod tests {
    use async_openai::types::Role;

    #[test]
    fn test_token_calculation() {
        use crate::conversation::ConversationCtx;

        use super::num_tokens_from_messages;

        println!("{}", Role::System.to_string());
        let mut ctx = ConversationCtx::default();
        ctx.add_system_message("You are a helpful, pattern-following assistant that translates corporate jargon into plain English.", None)
        .add_system_message("New synergies will help drive top-line growth.", Some("example_user".into()))
        .add_system_message("Things working well together will increase revenue.", Some("example_assistant".into()))
        .add_system_message("Let's circle back when we have more bandwidth to touch base on opportunities for increased leverage.", Some("example_user".into()))
        .add_system_message("Let's talk later when we're less busy about how to do better.", Some("example_assistant".into()))
        .add_user_message("This late pivot means we don't have time to boil the ocean for the client deliverable.", None);
        let nums = num_tokens_from_messages(&ctx, "gpt-3.5-turbo-0301").unwrap();
        assert_eq!(nums, 126);
    }
}
