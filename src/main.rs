#![recursion_limit = "256"]
#![deny(warnings)]

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
    id: Option<Id>,
    cfg: BotConfig,
}

impl Handler<'_> {
    async fn handle_message(&mut self, client: &Client, message: &Message<'_>) -> Result<()> {
        let (cmd, args) = match message.content.as_str().strip_prefix("eg!").and_then(|s| {
            let mut args = s.split_whitespace().filter(|a| !a.is_empty());
            args.next().map(|cmd| (cmd, args))
        }) {
            Some(p) => p,
            _ => return Ok(()),
        };

        macro_rules! match_command {
            (
                ($cmd:expr, $args:expr) {
                    $( $name:literal ($($param:pat),*) => $result:expr )*
                }
            ) => {{
                let mut args = $args;
                match (cmd) {
                    $(
                        $name => {
                            $(
                                let $param: &str = match Iterator::next(&mut args) {
                                    Some(p) => p,
                                    _ => ::anyhow::bail!("expected `{}` but none was provided", stringify!($param)),
                                };
                            )*
                            $result
                        }
                    )*
                    _ => ()
                }
            }}
        }

        match_command! {
            (cmd, args) {
                "mimic"() => self.mimic(client, message.channel_id).await?
                "follows"(word) => {
                    println!("{}", word);
                    self.create_list_message(client, message.channel_id, self.markov.what_follows(word)).await?;
                }
                "starts"() => {
                    self.create_list_message(client, message.channel_id, self.markov.what_starts()).await?;
                }
                "save"() => self.save(client, message.channel_id).await?
                "clean"() => self.clean(client, message).await?
                "learn"(channel, max) => {
                    let max = match max.to_lowercase().as_str() {
                        "full" => None,
                        s => Some(s.parse()?)
                    };
                    let learn_channel_id = channel.trim_start_matches("<#").trim_end_matches(">").parse()?;
                    self.learn_channel(client, message.channel_id, learn_channel_id, max).await?;
                }
            }
        }

        Ok(())
    }

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

    async fn save(&self, client: &Client, channel: Id) -> Result<()> {
        let result = save_markov(self.markov);
        let msg = match &result {
            Ok(s) => format!("Successfully saved ({})", file_size_to_string(*s)),
            Err(_) => String::from("Error saving :("),
        };
        client.create_message(channel, &msg).await?;
        result.and(Ok(()))
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

    async fn mimic(&mut self, client: &Client, channel: Id) -> Result<()> {
        client
            .create_message(
                channel,
                self.markov
                    .generate_sequence(&mut self.rng)
                    .fold(String::new(), |p, c| p + &c + " ")
                    .as_str(),
            )
            .await
    }

    async fn clean(&mut self, client: &Client, message: &Message<'_>) -> Result<()> {
        if self.is_admin_message(message) {
            let removed = self.markov.clean();
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

    async fn create_list_message(
        &mut self,
        client: &Client,
        channel: Id,
        iter: impl IntoIterator<Item = impl ToString>,
    ) -> Result<()> {
        let mut iter = iter.into_iter().peekable();
        let string = if iter.peek().is_none() {
            String::from("Nothing!")
        } else {
            iter.fold(String::new(), |p, c| p + &c.to_string() + " ")
        };

        client.create_message(channel, &string).await
    }

    async fn learn_channel(
        &mut self,
        client: &Client,
        return_channel: Id,
        channel: Id,
        max: Option<usize>,
    ) -> Result<()> {
        let mut oldest_id = None;
        let mut oldest_ts = None;
        let mut sum = 0;
        loop {
            let mut response = client
                .get_channel_messages(channel, oldest_id.take())
                .await?;
            let rate_limit_end = response.rate_limit_end();
            let messages = response.get_response().await?;

            sum += messages.len();
            for message in messages {
                self.remember(&message);
                if oldest_ts.map_or(true, |ts| message.timestamp < ts) {
                    oldest_id = Some(message.id);
                    oldest_ts = Some(message.timestamp);
                }
            }

            if oldest_id.is_none() || max.map(|m| sum >= m).unwrap_or(false) {
                client
                    .create_message(return_channel, &format!("learned from {} messages", sum))
                    .await?;
                break Ok(());
            }

            if let Some(time) = rate_limit_end {
                async_io::Timer::at(time).await;
            }
        }
    }

    fn remember(&mut self, message: &Message<'_>) {
        let words: Vec<_> = message
            .content
            .as_str()
            .split_whitespace()
            .filter_map(|s| {
                if !s.is_empty() {
                    if let Some(id) = s.strip_prefix("<@!").and_then(|s| s.strip_suffix('>')) {
                        for user in &message.mentions {
                            if Ok(user.id) == id.parse() {
                                return Some(format!("`{}#{}`", user.username, user.discriminator));
                            }
                        }
                        Some(format!("`<@!{}>`", id))
                    } else {
                        Some(String::from(s))
                    }
                } else {
                    None
                }
            })
            .collect();
        if words.len() >= 3 {
            self.markov.insert_sequence(words);
        }
    }

    fn is_admin_message(&self, message: &Message<'_>) -> bool {
        self.cfg
            .admins
            .iter()
            .any(|admin| *admin == message.author.id)
    }
}

fn save_markov(markov: &Markov) -> Result<u64> {
    let mut file = File::create("markov.dat")?;
    file.write_all(&bincode::serialize(markov)?)?;
    Ok(file.metadata()?.len())
}

fn file_size_to_string(size: u64) -> String {
    let mut size_f = size as f64;
    let suffixes = ["bytes", "kb", "mb", "gb", "tb"];
    for s in suffixes.iter() {
        if size_f / 1024.0 < 1.0 {
            return format!("{:.2}{}", size_f, s);
        }
        size_f /= 1024.0;
    }
    String::from("way too fricken big file!")
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
                    if self.id != Some(message.author.id) {
                        self.handle_wot(client, &message).await?;
                        self.handle_message(client, &message).await?;
                        if !self
                            .cfg
                            .channel_blacklist
                            .iter()
                            .any(|&bc| bc == message.channel_id)
                        {
                            self.remember(&message);
                        }
                    }
                    Ok(())
                }
                DispatchPayload::Ready(ready) => {
                    self.id = Some(ready.user.id);
                    for &chan in &self.cfg.announcement_channels {
                        client.create_message(chan, "Dispenser goin' up!").await?;
                    }
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
    admins: Vec<Id>,
    channel_blacklist: Vec<Id>,
    announcement_channels: Vec<Id>,
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

    while let Err(e) = run(&mut markov) {
        save_markov(&markov).unwrap();
        for cause in e.chain() {
            println!("{}", cause);
        }
    }

    save_markov(&markov).unwrap();
}
