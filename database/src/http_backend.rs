use crate::database::MailboxInfo;
use crate::filter::Filter;
use crate::message::{Message, State};
use crate::new_message::NewMessage;
use crate::Backend;
use anyhow::{anyhow, Context, Result};
use reqwest::Response;
use reqwest::{header::HeaderMap, Client};
use serde_json::json;

pub struct HttpBackend {
    client: Client,
    api_url: String,
}

impl HttpBackend {
    // Create a new HttpBackend instance
    pub fn new(api_url: String, api_token: Option<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        if let Some(token) = api_token {
            headers.append(
                "Authorization",
                format!("Bearer {token}")
                    .try_into()
                    .context("Invalid authorization token")?,
            );
        }
        Ok(Self {
            client: Client::builder()
                .default_headers(headers)
                .build()
                .context("Failed to create HTTP client")?,
            api_url,
        })
    }

    // Generate an error from a failed response
    async fn make_error(res: Response) -> anyhow::Error {
        let url = res.url().to_string();
        let status = res.status();
        match res.text().await {
            Ok(body) => anyhow!(
                "Request to {url} failed with status code {}\n\nResponse:{}",
                status,
                body
            ),
            Err(err) => err.into(),
        }
    }
}

impl Backend for HttpBackend {
    async fn add_messages(&self, messages: Vec<NewMessage>) -> Result<Vec<Message>> {
        let res = self
            .client
            .post(format!("{}/messages", self.api_url))
            .json(&messages)
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(Self::make_error(res).await);
        }
        res.json()
            .await
            .context("Error parsing add messages response")
    }

    async fn load_messages(&self, filter: Filter) -> Result<Vec<Message>> {
        let res = self
            .client
            .get(format!("{}/messages", self.api_url))
            .query(&filter)
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(Self::make_error(res).await);
        }
        res.json()
            .await
            .context("Error parsing load messages response")
    }

    async fn change_state(&self, filter: Filter, new_state: State) -> Result<Vec<Message>> {
        let res = self
            .client
            .put(format!("{}/messages", self.api_url))
            .query(&filter)
            .json(&json!({ "new_state": new_state }))
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(Self::make_error(res).await);
        }
        res.json()
            .await
            .context("Error parsing change state response")
    }

    async fn delete_messages(&self, filter: Filter) -> Result<Vec<Message>> {
        let res = self
            .client
            .delete(format!("{}/messages", self.api_url))
            .query(&filter)
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(Self::make_error(res).await);
        }
        res.json()
            .await
            .context("Error parsing delete messages response")
    }

    async fn load_mailboxes(&self, filter: Filter) -> Result<Vec<MailboxInfo>> {
        let res = self
            .client
            .get(format!("{}/mailboxes", self.api_url))
            .query(&filter)
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(Self::make_error(res).await);
        }
        res.json()
            .await
            .context("Error parsing load mailboxes response")
    }
}
