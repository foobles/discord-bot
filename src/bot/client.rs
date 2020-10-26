use anyhow::Result;
use futures::prelude::*;
use isahc::HttpClientBuilder;
use serde::{Deserialize, Serialize};

use crate::bot::types::*;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use std::str::FromStr;
use std::time::{Duration, Instant};

pub struct Client {
    http: isahc::HttpClient,
}

pub struct Response<T> {
    inner: ResponseInner,
    rate_limit_end: Option<Instant>,
    _phantom: PhantomData<T>,
}

enum ResponseInner {
    Response(isahc::Body),
    Bytes(Vec<u8>),
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

    pub async fn make_get_request<T>(&self, endpoint: &str) -> Result<Response<T>> {
        let response = self
            .http
            .get_async(dbg!(Self::get_discord_endpoint(endpoint)))
            .await?;

        let rate_limit_end = get_from_response::<usize, _>(&response, "X-RateLimit-Remaining")
            .and_then(|remaining| {
                if remaining == 0 {
                    let limit_end_after = get_from_response(&response, "X-RateLimit-Reset-After")?;
                    Some(Instant::now() + Duration::from_secs(limit_end_after))
                } else {
                    None
                }
            });

        Ok(Response {
            inner: ResponseInner::Response(response.into_body()),
            rate_limit_end: dbg!(rate_limit_end),
            _phantom: PhantomData,
        })
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

    pub async fn get_channel_messages<'a>(
        &self,
        channel: Id,
        message: Option<Id>,
    ) -> Result<Response<Vec<Message<'a>>>> {
        let mut endpoint = format!("/channels/{}/messages", channel);
        if let Some(message) = message {
            endpoint += &format!("?before={}", message);
        }
        self.make_get_request(&endpoint).await
    }
}

impl<T> Response<T> {
    pub fn rate_limit_end(&self) -> Option<Instant> {
        self.rate_limit_end
    }
}

impl<'a, 'de, T> Response<T>
where
    'a: 'de,
    T: Deserialize<'de> + 'a,
{
    pub async fn get_response(&'a mut self) -> Result<T>
    where
        'a: 'de,
        T: Deserialize<'de> + 'a,
    {
        if let ResponseInner::Response(body) = &mut self.inner {
            let mut bytes = Vec::new();
            body.read_to_end(&mut bytes).await?;
            self.inner = ResponseInner::Bytes(bytes);
        }
        match &self.inner {
            ResponseInner::Bytes(bytes) => {
                dbg!(String::from_utf8_lossy(bytes));
                Ok(serde_json::from_slice(bytes)?)
            }
            _ => unreachable!(),
        }
    }
}

impl<T: DeserializeOwned> Response<T> {
    pub fn get_response_owned(&mut self) -> Result<T> {
        Ok(match &mut self.inner {
            ResponseInner::Response(body) => serde_json::from_reader(body)?,
            ResponseInner::Bytes(bytes) => serde_json::from_slice(bytes)?,
        })
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

fn get_from_response<T: FromStr, U>(response: &http::Response<U>, q: &str) -> Option<T> {
    response.headers().get(q)?.to_str().ok()?.parse().ok()
}
