#![recursion_limit = "256"]
//#![deny(warnings)]

use anyhow::Result;

use crate::bot::client::Client;
use crate::bot::message::event::DispatchPayload;
use crate::markov::Markov;
use bot::types::*;
use bot::Bot;
use rand::Rng;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufReader, Write};

pub mod bot;
pub mod markov;
pub mod strings;

struct Handler<'a> {
    markov: &'a mut Markov,
    rng: rand::rngs::ThreadRng,
    id: Option<IdBuf>,
    cfg: BotConfig,
}

impl Handler<'_> {
    async fn add_emojis(&mut self, client: &Client, message: &Message<'_>) -> Result<()> {
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
        }
        Ok(())
    }

    async fn save(&self, client: &Client, channel: IdRef<'_>) -> Result<()> {
        let result = save_markov(self.markov);
        client
            .create_message(
                channel,
                match &result {
                    Ok(_) => "Successfully saved!",
                    Err(_) => "Error saving :(",
                },
            )
            .await?;
        result
    }

    async fn handle_wot(&mut self, client: &Client, message: &Message<'_>) -> Result<()> {
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
        Ok(())
    }

    async fn mimic(&mut self, client: &Client, channel: IdRef<'_>) -> Result<()> {
        client
            .create_message(
                channel,
                self.markov
                    .generate_sequence(&mut self.rng)
                    .fold(String::new(), |p, c| {
                        if c.starts_with('@') {
                            format!("{}`{}` ", p, c)
                        } else {
                            p + &c + " "
                        }
                    })
                    .as_str(),
            )
            .await
    }

    async fn clean(&mut self, client: &Client, message: &Message<'_>) -> Result<()> {
        if self
            .cfg
            .admins
            .iter()
            .any(|admin| *admin == message.author.id)
        {
            let removed = self.markov.clean(2);
            client
                .create_message(message.channel_id, &format!("Removed {} entries", removed))
                .await
        } else {
            client
                .create_message(
                    message.channel_id,
                    "Watch it, string bean. You aren't an admin",
                )
                .await
        }
    }

    async fn what_follows(
        &mut self,
        client: &Client,
        channel: IdRef<'_>,
        word: &str,
    ) -> Result<()> {
        let follows = self.markov.what_follows(word);
        if follows.is_empty() {
            client.create_message(channel, "Nothing does!").await
        } else {
            client
                .create_message(
                    channel,
                    follows
                        .into_iter()
                        .fold(String::new(), |p, c| p + &c + "\n")
                        .as_str(),
                )
                .await
        }
    }

    fn remember(&mut self, string: &str) {
        self.markov.insert_sequence(
            string
                .split_whitespace()
                .filter(|s| !s.is_empty())
                .map(String::from),
        )
    }
}

fn save_markov(markov: &Markov) -> Result<()> {
    Ok(File::create("markov.dat")?.write_all(&bincode::serialize(markov)?)?)
}

impl bot::AsyncDispatchHandler for Handler<'_> {
    fn handle_message<'a>(
        &'a mut self,
        payload: DispatchPayload<'a>,
        client: &'a Client,
    ) -> bot::AsyncDispatchFuture<'a> {
        Box::pin(async move {
            match payload {
                DispatchPayload::MessageCreate(message) => {
                    self.add_emojis(client, &message).await?;
                    if self.id.as_ref().map(Id::as_ref) != Some(message.author.id) {
                        self.handle_wot(client, &message).await?;
                        match message.content.as_str().trim() {
                            "eg!mimic" => self.mimic(client, message.channel_id).await?,
                            "eg!save" => self.save(client, message.channel_id).await?,
                            "eg!debug" => eprintln!("{:#?}", self.markov),
                            s if s.starts_with("eg!follows ") => {
                                self.what_follows(
                                    client,
                                    message.channel_id,
                                    message.content.as_str()[11..].trim(),
                                )
                                .await?
                            }
                            s if !self
                                .cfg
                                .channel_blacklist
                                .iter()
                                .any(|c| *c == message.channel_id) =>
                            {
                                self.remember(s);
                            }
                            _ => (),
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
    admins: Vec<IdBuf>,
    channel_blacklist: Vec<IdBuf>,
}

fn run(markov: &mut Markov) -> Result<()> {
    let bot_cfg: BotConfig = serde_json::from_reader(BufReader::new(File::open("bot.json")?))?;

    let bot = Bot::new(bot_cfg.token.clone(), bot_cfg.intents);
    bot.run(Handler {
        markov,
        rng: rand::thread_rng(),
        id: None,
        cfg: bot_cfg,
    })
}

fn main() {
    let mut markov = File::open("markov.dat")
        .map_err(bincode::Error::from)
        .and_then(bincode::deserialize_from)
        .map_err(|e| {
            eprintln!("{}", e);
            e
        })
        .unwrap_or_else(|_| Markov::new());

    if let Err(e) = run(&mut markov) {
        for cause in e.chain() {
            println!("{}", cause);
        }
    }

    save_markov(&markov).unwrap();
}
