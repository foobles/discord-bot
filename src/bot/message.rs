use serde::{Deserialize, Serialize};

use crate::bot::types::*;

pub mod command {
    use serde::ser::SerializeStruct;
    use serde::Serializer;

    use super::*;

    pub trait Command: Serialize {
        const OP: u8;
    }

    pub struct CommandSerializer<C: Command>(pub C);

    impl<T: Command> Serialize for CommandSerializer<T> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut s = serializer.serialize_struct("Command", 2)?;
            s.serialize_field("op", &T::OP)?;
            s.serialize_field("d", &self.0)?;
            s.end()
        }
    }

    #[derive(Serialize)]
    pub struct Heartbeat(pub Option<Sequence>);

    impl Command for Heartbeat {
        const OP: u8 = 1;
    }

    #[derive(Serialize)]
    pub struct Identify {
        pub token: TokenBuf,
        pub properties: ConnectionProperties,
        pub intents: Intents,
        pub compress: Option<bool>,
        pub large_threshold: Option<u8>,
    }

    impl Command for Identify {
        const OP: u8 = 2;
    }

    #[derive(Serialize)]
    pub struct ConnectionProperties {
        #[serde(rename = "$os")]
        pub os: String,
        #[serde(rename = "$browser")]
        pub browser: String,
        #[serde(rename = "$device")]
        pub device: String,
    }

    #[derive(Serialize)]
    pub struct UpdateStatus {
        pub since: Option<i32>,
        pub status: Status,
        pub afk: bool,
        pub activities: Option<Never>, // omitted for now -- might implement later
    }

    impl Command for UpdateStatus {
        const OP: u8 = 3;
    }

    #[derive(Serialize)]
    pub struct Resume {
        pub token: TokenBuf,
        pub session_id: String,
        pub seq: Sequence,
    }

    impl Command for Resume {
        const OP: u8 = 6;
    }

    #[derive(Serialize)]
    pub enum Never {}
}

pub mod event {
    use std::convert::TryFrom;

    use serde::de::{Error, Unexpected};
    use serde::Deserializer;
    use serde_json::value::RawValue;

    use super::*;

    #[derive(Deserialize)]
    #[serde(try_from = "RawEvent")]
    pub enum Event<'a> {
        #[serde(borrow)]
        Dispatch(Dispatch<'a>),
        Reconnect,
        InvalidSession(bool),
        Hello(Hello),
        HeartbeatAck,
        // Heartbeat?
    }

    pub struct Dispatch<'a> {
        pub seq: Sequence,
        pub payload: DispatchPayload<'a>,
    }

    #[derive(Debug)]
    pub enum DispatchPayload<'a> {
        MessageCreate(Message<'a>),
        // more to be added later
        Ready(Ready<'a>),
        TypingStart(TypingStart<'a>),
    }

    #[derive(Deserialize)]
    pub struct Hello {
        pub heartbeat_interval: u64,
    }

    #[derive(Deserialize, Debug)]
    pub struct Ready<'a> {
        #[serde(borrow)]
        pub user: User<'a>,
        pub session_id: &'a str,
    }

    #[derive(Deserialize, Debug)]
    pub struct TypingStart<'a> {
        channel_id: Id,
        guild_id: Option<Id>,
        user_id: Id,
        timestamp: i32,

        #[serde(borrow)]
        member: Option<Member<'a>>,
    }

    #[derive(Deserialize)]
    struct RawEvent<'a> {
        op: u8,
        t: Option<&'a str>,
        s: Option<Sequence>,

        #[serde(borrow)]
        d: &'a RawValue,
    }

    impl<'a> TryFrom<RawEvent<'a>> for Event<'a> {
        type Error = serde_json::Error;

        fn try_from(raw: RawEvent<'a>) -> Result<Self, Self::Error> {
            fn get_dispatch<'de: 'a, 'a, D>(
                de: D,
                t: &str,
                seq: Sequence,
            ) -> Result<Dispatch<'a>, <Event as TryFrom<RawEvent>>::Error>
            where
                D: Deserializer<'de, Error = serde_json::Error>,
            {
                let payload = match t {
                    "MESSAGE_CREATE" => {
                        Message::deserialize(de).map(DispatchPayload::MessageCreate)
                    }
                    "READY" => Ready::deserialize(de).map(DispatchPayload::Ready),
                    "TYPING_START" => {
                        TypingStart::deserialize(de).map(DispatchPayload::TypingStart)
                    }
                    s => Err(serde_json::Error::invalid_value(
                        Unexpected::Str(s),
                        &"valid gateway message type",
                    )),
                }?;
                Ok(Dispatch { seq, payload })
            }
            fn deserialize_null<'de, D>(de: D, ret: Event) -> Result<Event, D::Error>
            where
                D: Deserializer<'de>,
            {
                serde_json::Value::deserialize(de).and_then(|v| {
                    if v == serde_json::Value::Null {
                        Ok(ret)
                    } else {
                        Err(D::Error::invalid_value(
                            Unexpected::Other("non-null payload"),
                            &"null",
                        ))
                    }
                })
            }

            let mut de = serde_json::Deserializer::from_str(raw.d.get());

            const OP_DISPATCH: u8 = 0;
            const OP_RECONNECT: u8 = 7;
            const OP_INVALID_SESSION: u8 = 9;
            const OP_HELLO: u8 = 10;
            const OP_HEARTBEAT_ACK: u8 = 11;

            let ret = match raw.op {
                OP_RECONNECT => deserialize_null(&mut de, Event::Reconnect),
                OP_INVALID_SESSION => bool::deserialize(&mut de).map(Event::InvalidSession),
                OP_HELLO => Hello::deserialize(&mut de).map(Event::Hello),
                OP_HEARTBEAT_ACK => deserialize_null(&mut de, Event::HeartbeatAck),
                OP_DISPATCH => get_dispatch(
                    &mut de,
                    raw.t.ok_or_else(|| serde_json::Error::missing_field("t"))?,
                    raw.s.ok_or_else(|| serde_json::Error::missing_field("s"))?,
                )
                .map(Event::Dispatch),
                n => Err(serde_json::Error::invalid_value(
                    Unexpected::Unsigned(n as u64),
                    &"valid gateway event code (0-11)",
                )),
            }?;
            de.end()?;
            Ok(ret)
        }
    }
}
