use std::sync::{MutexGuard, PoisonError};
use std::{collections::HashMap, sync::Mutex};

use async_openai::types::{Role, ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs};
use serenity::model::prelude::{Message, UserId};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub role: Role,
    pub message: Message,
}

type Messages = Vec<ConversationMessage>;
type UserMessagesMap = HashMap<UserId, Messages>;
#[derive(Debug)]
pub struct ConversationCache {
    pub map: Mutex<UserMessagesMap>,
    pub max_length: usize,
}

impl Default for ConversationCache {
    fn default() -> Self {
        Self {
            map: Default::default(),
            max_length: 20,
        }
    }
}

#[derive(Error, Debug)]
pub enum ConversationCacheError {
    #[error("Channel not found")]
    ChannelNotFound,
    #[error("Failed to acquire lock on mutex, this should never happen.")]
    MutexPanic,
}

impl From<PoisonError<MutexGuard<'_, UserMessagesMap>>> for ConversationCacheError {
    fn from(_: PoisonError<MutexGuard<'_, UserMessagesMap>>) -> Self {
        Self::MutexPanic
    }
}

impl ConversationCache {
    pub fn add_message(
        &self,
        user_id: UserId,
        role: Role,
        message: Message,
    ) -> Result<(), ConversationCacheError> {
        let mut map = self.map.lock()?;
        let messages = map
            .entry(user_id)
            .or_insert_with(|| Vec::with_capacity(self.max_length));
        messages.push(ConversationMessage {role, message});
        if messages.len() > self.max_length {
            messages.remove(0);
        }
        Ok(())
    }

    pub fn get_messages(&self, user_id: UserId) -> Result<Messages, ConversationCacheError> {
        let map = self.map.lock()?;
        Ok(map.get(&user_id).cloned().unwrap_or(vec![]))
    }
}

impl Into<ChatCompletionRequestMessage> for ConversationMessage {
    fn into(self) -> ChatCompletionRequestMessage {
        ChatCompletionRequestMessageArgs::default()
            .role(self.role)
            .content(self.message.content)
            .build()
            .unwrap()
    }
}