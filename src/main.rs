#![recursion_limit = "256"]
//#![deny(warnings)]

use anyhow::Result;

use crate::bot::client::Client;
use crate::bot::message::event::DispatchPayload;
use bot::types::*;
use bot::Bot;
use rand::Rng;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;

pub mod bot;
pub mod strings;

struct Handler {
    rng: rand::rngs::ThreadRng,
    id: Option<IdBuf>,
}

impl bot::AsyncDispatchHandler for Handler {
    fn handle_message<'a>(
        &'a mut self,
        payload: DispatchPayload<'a>,
        client: &'a Client,
    ) -> bot::AsyncDispatchFuture<'a> {
        Box::pin(async move {
            match payload {
                DispatchPayload::MessageCreate(message) => {
                    let mut emoji = None;
                    if self.rng.gen_ratio(1, 50) {
                        emoji = Some("bonk:756521659938111602");
                    } else if self.rng.gen_ratio(1, 200) {
                        emoji = Some("💦");
                    }
                    if let Some(emoji) = emoji {
                        client
                            .create_reaction(message.channel_id, message.id, emoji)
                            .await?;
                    } else if self.id.as_ref().map(Id::as_ref) != Some(message.author.id) {
                        if message
                            .content
                            .as_str()
                            .split_whitespace()
                            .any(|w| w.to_lowercase() == "wot")
                        {
                            client
                                .create_message(message.channel_id, "u wot m8")
                                .await?;
                        }
                    }
                    Ok(())
                }
                DispatchPayload::Ready(ready) => {
                    self.id = Some(ready.user.id.into_owned());
                    Ok(())
                }
                _ => Ok(()),
            }
        })
    }
}

#[derive(Deserialize)]
struct BotConfig {
    token: TokenBuf,
    intents: Intents,
}

fn run() -> Result<()> {
    let bot_cfg: BotConfig = serde_json::from_reader(BufReader::new(File::open("bot.json")?))?;

    let bot = Bot::new(bot_cfg.token, bot_cfg.intents);
    bot.run(Handler {
        rng: rand::thread_rng(),
        id: None,
    })
}

fn main() {
    if let Err(e) = run() {
        for cause in e.chain() {
            println!("{}", cause);
        }
    }
}
