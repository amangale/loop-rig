use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

pub struct ModelClient {
    http: reqwest::Client,
    base_url: String,
    model: String,
}

impl ModelClient {
    pub fn new() -> Self {
        let base_url = std::env::var("LOOP_RIG_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434/v1".into());
        let model = std::env::var("LOOP_RIG_MODEL")
            .unwrap_or_else(|_| "qwen2.5:14b".into());
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("failed to build http client");
        Self { http, base_url, model }
    }

    pub async fn chat(&self, messages: &[ChatMessage]) -> Result<String> {
        let req = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            temperature: 0.0,
        };
        let resp = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .json(&req)
            .send()
            .await
            .context("request to model failed")?;
        let resp = resp.error_for_status().context("model endpoint returned error status")?;
        let body: ChatResponse = resp.json().await.context("parsing model response body")?;
        body.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .context("model response contained no choices")
    }
}