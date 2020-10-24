use anyhow::Result;
use isahc::prelude::*;
use isahc::HttpClientBuilder;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::bot::types::*;

pub struct Client {
    pub(super) http: isahc::HttpClient,
}

impl Client {
    const DISCORD_ROOT: &'static str = "https://discord.com/api/";

    pub fn new(auth: &Token) -> Self {
        Client {
            http: HttpClientBuilder::new()
                .default_headers(&[
                    ("Authorization", format!("Bot {}", auth).as_str()),
                    ("Content-Type", "application/json"),
                ])
                .build()
                .expect("isahc client initialization"),
        }
    }

    fn get_discord_endpoint(endpoint: &str) -> String {
        format!("{}/{}", Self::DISCORD_ROOT, endpoint)
    }

    pub async fn make_get_request<T>(&self, endpoint: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        Ok(self
            .http
            .get_async(Self::get_discord_endpoint(endpoint))
            .await?
            .json()?)
    }

    pub async fn make_put_request(&self, endpoint: &str, body: String) -> Result<()> {
        let response = self
            .http
            .put_async(Self::get_discord_endpoint(endpoint), dbg!(body))
            .await?;
        dbg!(response);
        Ok(())
    }

    pub async fn make_post_request(&self, endpoint: &str, body: String) -> Result<()> {
        let response = self
            .http
            .post_async(Self::get_discord_endpoint(endpoint), dbg!(body))
            .await?;
        dbg!(response);
        Ok(())
    }

    pub async fn create_message(&self, channel_id: Id, content: &str) -> Result<()> {
        #[derive(Serialize)]
        struct CreateMessage<'a> {
            content: &'a str,
        }
        self.make_post_request(
            &format!("/channels/{}/messages", channel_id),
            serde_json::to_string(&CreateMessage { content })
                .expect("Cannot format message to create "),
        )
        .await?;

        Ok(())
    }

    pub async fn create_reaction(&self, channel: Id, message: Id, emoji: &str) -> Result<()> {
        let encoded_emoji = url_encode(emoji);

        let endpoint = format!(
            "/channels/{}/messages/{}/reactions/{}/@me",
            channel, message, encoded_emoji
        );
        self.make_put_request(&dbg!(endpoint), String::default())
            .await?;
        Ok(())
    }
}

fn url_encode(data: &str) -> String {
    data.bytes()
        .flat_map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                vec![b as char]
            } else {
                format!("%{:2X}", b).chars().collect()
            }
        })
        .collect()
}
