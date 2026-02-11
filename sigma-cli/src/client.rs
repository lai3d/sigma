use anyhow::{Context, Result};
use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::config::Config;

pub struct SigmaClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl SigmaClient {
    pub fn new(config: &Config) -> Result<Self> {
        let client = Client::new();
        Ok(SigmaClient {
            client,
            base_url: config.api_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn add_auth(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        if let Some(ref key) = self.api_key {
            builder.header("X-Api-Key", key)
        } else {
            builder
        }
    }

    async fn handle_error(resp: Response) -> anyhow::Error {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if let Ok(err) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(msg) = err.get("error").and_then(|e| e.as_str()) {
                return anyhow::anyhow!("API error ({}): {}", status, msg);
            }
        }
        anyhow::anyhow!("API error ({}): {}", status, body)
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .add_auth(self.client.get(self.url(path)))
            .send()
            .await
            .context("Failed to connect to API")?;
        if !resp.status().is_success() {
            return Err(Self::handle_error(resp).await);
        }
        resp.json::<T>().await.context("Failed to parse response")
    }

    pub async fn get_text(&self, path: &str) -> Result<(String, Option<String>)> {
        let resp = self
            .add_auth(self.client.get(self.url(path)))
            .send()
            .await
            .context("Failed to connect to API")?;
        if !resp.status().is_success() {
            return Err(Self::handle_error(resp).await);
        }
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let text = resp.text().await.context("Failed to read response body")?;
        Ok((text, content_type))
    }

    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let resp = self
            .add_auth(self.client.post(self.url(path)))
            .json(body)
            .send()
            .await
            .context("Failed to connect to API")?;
        if !resp.status().is_success() {
            return Err(Self::handle_error(resp).await);
        }
        resp.json::<T>().await.context("Failed to parse response")
    }

    pub async fn post_empty<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .add_auth(self.client.post(self.url(path)))
            .send()
            .await
            .context("Failed to connect to API")?;
        if !resp.status().is_success() {
            return Err(Self::handle_error(resp).await);
        }
        resp.json::<T>().await.context("Failed to parse response")
    }

    pub async fn put<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let resp = self
            .add_auth(self.client.put(self.url(path)))
            .json(body)
            .send()
            .await
            .context("Failed to connect to API")?;
        if !resp.status().is_success() {
            return Err(Self::handle_error(resp).await);
        }
        resp.json::<T>().await.context("Failed to parse response")
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let resp = self
            .add_auth(self.client.delete(self.url(path)))
            .send()
            .await
            .context("Failed to connect to API")?;
        if resp.status() == StatusCode::NO_CONTENT || resp.status().is_success() {
            return Ok(());
        }
        Err(Self::handle_error(resp).await)
    }

    pub async fn delete_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .add_auth(self.client.delete(self.url(path)))
            .send()
            .await
            .context("Failed to connect to API")?;
        if !resp.status().is_success() {
            return Err(Self::handle_error(resp).await);
        }
        resp.json::<T>().await.context("Failed to parse response")
    }
}
