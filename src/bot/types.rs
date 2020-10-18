use crate::strings::StrCow;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq)]
pub struct Token<S: AsRef<str>>(pub S);
pub type TokenRef<'a> = Token<&'a str>;
pub type TokenBuf = Token<String>;
impl<S: AsRef<str> + ToOwned + ?Sized> Token<&S>
where
    S::Owned: AsRef<str>,
{
    pub fn into_owned(self) -> Token<S::Owned> {
        Token(self.0.to_owned())
    }
}

impl<S: AsRef<str>> Token<S> {
    pub fn as_ref(&self) -> TokenRef {
        Token(self.0.as_ref())
    }
}

impl<S: AsRef<str>, R: AsRef<str>> PartialEq<Token<R>> for Token<S> {
    fn eq(&self, other: &Token<R>) -> bool {
        self.0.as_ref() == other.0.as_ref()
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq)]
pub struct Id<S: AsRef<str>>(pub S);
pub type IdRef<'a> = Id<&'a str>;
pub type IdBuf = Id<String>;
impl<S: AsRef<str> + ToOwned + ?Sized> Id<&S>
where
    S::Owned: AsRef<str>,
{
    pub fn into_owned(self) -> Id<S::Owned> {
        Id(self.0.to_owned())
    }
}

impl<S: AsRef<str>> Id<S> {
    pub fn as_ref(&self) -> IdRef {
        Id(self.0.as_ref())
    }
}

impl<S: AsRef<str>, R: AsRef<str>> PartialEq<Id<R>> for Id<S> {
    fn eq(&self, other: &Id<R>) -> bool {
        self.0.as_ref() == other.0.as_ref()
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

    #[serde(borrow)]
    pub id: IdRef<'a>,

    #[serde(borrow)]
    pub channel_id: IdRef<'a>,

    #[serde(borrow)]
    pub author: User<'a>,
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
    user: Option<User<'a>>,
    #[serde(borrow)]
    nick: Option<StrCow<'a>>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct User<'a> {
    #[serde(borrow)]
    pub id: IdRef<'a>,
    pub username: &'a str, // might need Cow
}
