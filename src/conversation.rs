use std::{
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
    sync::{Mutex, MutexGuard, PoisonError}, collections::VecDeque,
};

use async_openai::{
    error::OpenAIError,
    types::{ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs, Role},
};
use lru::LruCache;
use serenity::model::prelude::UserId;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub role: Role,
    pub message: String,
}

type UserMessagesMap = LruCache<UserId, ConversationCtx>;
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
        message: &str,
        name: Option<String>,
    ) -> Result<(), ConversationCacheError> {
        let mut map = self.map.lock()?;
        let ctx = map.get_or_insert_mut(user_id, ConversationCtx::default);

        ctx.add_message(role, message, name);
        if ctx.value.len() > self.max_conversation_length {
            ctx.value.remove(0);
        }
        Ok(())
    }

    pub fn get_messages(&self, user_id: UserId) -> Result<ConversationCtx, ConversationCacheError> {
        let mut map = self.map.lock()?;
        Ok(map.get(&user_id).cloned().unwrap_or_default())
    }
}

impl TryFrom<ConversationMessage> for ChatCompletionRequestMessage {
    type Error = OpenAIError;

    fn try_from(val: ConversationMessage) -> Result<Self, Self::Error> {
        ChatCompletionRequestMessageArgs::default()
            .role(val.role)
            .content(val.message)
            .build()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConversationCtx {
    pub value: VecDeque<ChatCompletionRequestMessage>,
}

impl From<ConversationCtx> for VecDeque<ChatCompletionRequestMessage> {
    fn from(ctx: ConversationCtx) -> Self {
        ctx.value
    }
}

impl Deref for ConversationCtx {
    type Target = VecDeque<ChatCompletionRequestMessage>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl DerefMut for ConversationCtx {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl ConversationCtx {
    pub fn add_user_message(&mut self, message: &str, name: Option<String>) -> &mut Self {
        self.add_message(Role::User, message, name);
        self
    }

    pub fn add_system_message(&mut self, message: &str, name: Option<String>) -> &mut Self {
        self.add_message(Role::System, message, name);
        self
    }

    pub fn add_assistant_message(&mut self, message: &str, name: Option<String>) -> &mut Self {
        self.add_message(Role::Assistant, message, name);
        self
    }

    pub fn add_message(&mut self, role: Role, message: &str, name: Option<String>) -> &mut Self {
        let mut binding = ChatCompletionRequestMessageArgs::default();
        let mut arg = binding.role(role).content(message);
        if let Some(name) = name {
            arg = arg.name(name);
        }
        let message = arg.build().expect("Unreachable!");
        self.value.push_back(message);
        self
    }
}
