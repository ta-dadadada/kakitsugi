use std::{collections::BTreeSet, fmt, str::FromStr};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const MAX_AUTHOR_CHARS: usize = 128;
pub const MAX_TITLE_CHARS: usize = 512;
pub const MAX_BODY_BYTES: usize = 1024 * 1024;
pub const MAX_TAG_CHARS: usize = 64;
pub const MAX_TAGS: usize = 16;
pub const DEFAULT_LIMIT: u32 = 50;
pub const MAX_LIMIT: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThreadStatus {
    Open,
    Closed,
}

impl ThreadStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }
}

impl fmt::Display for ThreadStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ThreadStatus {
    type Err = ValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "open" => Ok(Self::Open),
            "closed" => Ok(Self::Closed),
            _ => Err(ValidationError::new(
                "status must be either 'open' or 'closed'",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Board {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BoardsResponse {
    pub items: Vec<Board>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ThreadRecord {
    pub id: String,
    pub board_id: String,
    pub title: String,
    pub status: ThreadStatus,
    pub author: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Post {
    pub id: String,
    pub thread_id: String,
    pub author: String,
    pub body: String,
    pub created_at: String,
    pub event_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Event {
    pub id: i64,
    pub kind: String,
    pub thread_id: String,
    pub post_id: Option<String>,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EventsResponse {
    pub items: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PostPage {
    pub items: Vec<Post>,
    pub limit: u32,
    pub after: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ThreadDetail {
    pub thread: ThreadRecord,
    pub posts: Vec<Post>,
    pub limit: u32,
    pub after: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateThreadInput {
    pub title: String,
    pub author: String,
    pub body: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ValidCreateThread {
    pub title: String,
    pub author: String,
    pub body: String,
    pub tags: Vec<String>,
}

impl CreateThreadInput {
    pub(crate) fn validate(self) -> Result<ValidCreateThread, ValidationError> {
        Ok(ValidCreateThread {
            title: required_chars("title", self.title, MAX_TITLE_CHARS)?,
            author: required_chars("author", self.author, MAX_AUTHOR_CHARS)?,
            body: required_body(self.body)?,
            tags: normalize_tags(self.tags)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReplyInput {
    pub author: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ValidReply {
    pub author: String,
    pub body: String,
}

impl ReplyInput {
    pub(crate) fn validate(self) -> Result<ValidReply, ValidationError> {
        Ok(ValidReply {
            author: required_chars("author", self.author, MAX_AUTHOR_CHARS)?,
            body: required_body(self.body)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateThreadInput {
    pub actor: String,
    pub status: Option<ThreadStatus>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub(crate) struct ValidUpdateThread {
    pub actor: String,
    pub status: Option<ThreadStatus>,
    pub tags: Option<Vec<String>>,
}

impl UpdateThreadInput {
    pub(crate) fn validate(self) -> Result<ValidUpdateThread, ValidationError> {
        if self.status.is_none() && self.tags.is_none() {
            return Err(ValidationError::new("status or tags is required"));
        }
        Ok(ValidUpdateThread {
            actor: required_chars("actor", self.actor, MAX_AUTHOR_CHARS)?,
            status: self.status,
            tags: self.tags.map(normalize_tags).transpose()?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListThreadsInput {
    pub status: Option<ThreadStatus>,
    pub tag: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

impl Default for ListThreadsInput {
    fn default() -> Self {
        Self {
            status: None,
            tag: None,
            limit: DEFAULT_LIMIT,
            offset: 0,
        }
    }
}

impl ListThreadsInput {
    pub(crate) fn validate(mut self) -> Result<Self, ValidationError> {
        validate_limit(self.limit)?;
        self.tag = self.tag.map(normalize_tag).transpose()?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchInput {
    #[serde(alias = "q")]
    pub query: String,
    pub status: Option<ThreadStatus>,
    pub tag: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

impl SearchInput {
    pub(crate) fn validate(mut self) -> Result<Self, ValidationError> {
        self.query = required_chars("query", self.query, MAX_TITLE_CHARS)?;
        validate_limit(self.limit)?;
        self.tag = self.tag.map(normalize_tag).transpose()?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateThreadResponse {
    pub thread: ThreadRecord,
    pub initial_post: Post,
    pub event_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReplyResponse {
    pub post: Post,
    pub event_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateThreadResponse {
    pub thread: ThreadRecord,
    pub changed: bool,
    pub event_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{message}")]
pub struct ValidationError {
    message: String,
}

impl ValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub fn default_limit() -> u32 {
    DEFAULT_LIMIT
}

pub(crate) fn validate_limit(limit: u32) -> Result<(), ValidationError> {
    if (1..=MAX_LIMIT).contains(&limit) {
        Ok(())
    } else {
        Err(ValidationError::new("limit must be between 1 and 100"))
    }
}

fn required_chars(field: &str, value: String, max_chars: usize) -> Result<String, ValidationError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ValidationError::new(format!("{field} must not be empty")));
    }
    if value.chars().count() > max_chars {
        return Err(ValidationError::new(format!(
            "{field} must be at most {max_chars} characters"
        )));
    }
    Ok(value)
}

fn required_body(value: String) -> Result<String, ValidationError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ValidationError::new("body must not be empty"));
    }
    if value.len() > MAX_BODY_BYTES {
        return Err(ValidationError::new("body must be at most 1 MiB"));
    }
    Ok(value)
}

fn normalize_tag(value: String) -> Result<String, ValidationError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ValidationError::new("tag must not be empty"));
    }
    if value.chars().count() > MAX_TAG_CHARS {
        return Err(ValidationError::new("tag must be at most 64 characters"));
    }
    Ok(value)
}

fn normalize_tags(values: Vec<String>) -> Result<Vec<String>, ValidationError> {
    if values.len() > MAX_TAGS {
        return Err(ValidationError::new("at most 16 tags are allowed"));
    }
    let mut tags = BTreeSet::new();
    for value in values {
        tags.insert(normalize_tag(value)?);
    }
    Ok(tags.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_normalizes_and_sorts_tags() {
        let valid = CreateThreadInput {
            title: " Topic ".into(),
            author: " claude ".into(),
            body: " body ".into(),
            tags: vec!["z".into(), "a".into(), "z".into()],
        }
        .validate()
        .unwrap();

        assert_eq!(valid.title, "Topic");
        assert_eq!(valid.tags, vec!["a", "z"]);
    }

    #[test]
    fn empty_required_value_is_rejected() {
        let result = ReplyInput {
            author: " ".into(),
            body: "body".into(),
        }
        .validate();
        assert!(result.is_err());
    }
}
