use std::{
    num::NonZeroUsize,
    sync::{Mutex, MutexGuard, PoisonError},
};

use async_openai::types::{ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs, Role};
use lru::LruCache;
use serenity::model::prelude::{Message, UserId};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub role: Role,
    pub message: Message,
}

type Messages = Vec<ConversationMessage>;
type UserMessagesMap = LruCache<UserId, Messages>;
#[derive(Debug)]
pub struct ConversationCache {
    pub map: Mutex<UserMessagesMap>,
    pub max_conversation_length: usize,
    pub max_keys_length: usize,
}

impl Default for ConversationCache {
    fn default() -> Self {
        let max_keys_length = 256;
        let map = Mutex::new(LruCache::new(
            NonZeroUsize::new(max_keys_length).expect("Unreachable!"),
        ));
        Self {
            max_conversation_length: 20,
            map,
            max_keys_length,
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
        let messages =
            map.get_or_insert_mut(user_id, || Vec::with_capacity(self.max_conversation_length));

        messages.push(ConversationMessage { role, message });
        if messages.len() > self.max_conversation_length {
            messages.remove(0);
        }
        Ok(())
    }

    pub fn get_messages(&self, user_id: UserId) -> Result<Messages, ConversationCacheError> {
        let mut map = self.map.lock()?;
        Ok(map.get(&user_id).cloned().unwrap_or_default())
    }
}

impl From<ConversationMessage> for ChatCompletionRequestMessage {
    fn from(val: ConversationMessage) -> Self {
        ChatCompletionRequestMessageArgs::default()
            .role(val.role)
            .content(val.message.content)
            .build()
            .unwrap()
    }
}
