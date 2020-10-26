use crate::strings::StrCow;
use chrono::{DateTime, Utc};
use serde::{de::Error, Deserialize, Deserializer, Serialize};
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TokenBuf(String);

impl<T: Into<String>> From<T> for TokenBuf {
    fn from(s: T) -> Self {
        TokenBuf(s.into())
    }
}

impl Deref for TokenBuf {
    type Target = Token;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.0.as_str() as *const str as *const Token) }
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct Token(str);

impl Token {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Serialize, Copy, Clone, Debug, PartialEq, Eq)]
#[serde(into = "String")]
pub struct Id(u64);

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        StrCow::deserialize(deserializer)
            .and_then(|s| Ok(Id(s.as_str().parse().map_err(D::Error::custom)?)))
    }
}

impl FromStr for Id {
    type Err = <u64 as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Id(s.parse()?))
    }
}

impl From<Id> for String {
    fn from(id: Id) -> Self {
        id.0.to_string()
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&String::from(*self))
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct Sequence(pub usize);

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Online,
    Dnd,
    Idle,
    Invisible,
    Offline,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message<'a> {
    #[serde(borrow)]
    pub content: StrCow<'a>,

    pub id: Id,
    pub channel_id: Id,

    #[serde(deserialize_with = "deserialize_datetime_into_millis")]
    pub timestamp: i64,

    #[serde(borrow)]
    pub author: User<'a>,

    #[serde(borrow)]
    pub mentions: Vec<User<'a>>,
}

#[derive(Serialize, Deserialize, Copy, Clone)]
#[serde(rename_all = "snake_case")]
#[repr(u16)]
pub enum Intent {
    Guilds = 1 << 0,
    GuildMembers = 1 << 1,
    GuildBans = 1 << 2,
    GuildEmojis = 1 << 3,
    GuildIntegrations = 1 << 4,
    GuildWebhooks = 1 << 5,
    GuildInvites = 1 << 6,
    GuildVoiceStates = 1 << 7,
    GuildPresences = 1 << 8,
    GuildMessages = 1 << 9,
    GuildMessageReactions = 1 << 10,
    GuildMessageTyping = 1 << 11,
    DirectMessages = 1 << 12,
    DirectMessageReactions = 1 << 13,
    DirectMessageTyping = 1 << 14,
}

impl Intent {
    pub const fn and(self, other: Intent) -> Intents {
        Intents(self as u16 | other as u16)
    }
}

#[derive(Serialize, Deserialize, Copy, Clone)]
#[serde(from = "Vec<Intent>")]
pub struct Intents(u16);

impl<I: IntoIterator<Item = Intent>> From<I> for Intents {
    fn from(iter: I) -> Self {
        iter.into_iter().fold(Intents(0), |i, s| i.and(s))
    }
}

impl Intents {
    pub const fn and(self, other: Intent) -> Intents {
        Intents(self.0 | other as u16)
    }
}

impl From<Intent> for Intents {
    fn from(intent: Intent) -> Intents {
        Intents(intent as u16)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Member<'a> {
    #[serde(borrow)]
    pub user: Option<User<'a>>,
    #[serde(borrow)]
    pub nick: Option<StrCow<'a>>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct User<'a> {
    pub id: Id,
    pub username: &'a str, // might need Cow
    pub discriminator: &'a str,
}

fn deserialize_datetime_into_millis<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    DateTime::<Utc>::deserialize(deserializer).map(|dt| dt.timestamp_millis())
}
