use std::net::TcpStream;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use async_io::{Async, Timer};
use async_tungstenite::{tungstenite::Message, WebSocketStream};
use futures::{future::FusedFuture, prelude::*, select};
use serde::Deserialize;
use std::pin::Pin;
use url::Url;

use message::{command::*, event::*};
use types::*;

pub mod client;
pub mod message;
pub mod types;

use client::Client;

type WebSocket = WebSocketStream<async_tungstenite::async_tls::ClientStream<Async<TcpStream>>>;

macro_rules! expect_message_or_bail {
    ($stream:expr, $user_pat:pat = $message_type:ident => $result:expr) => {{
        let next = $stream.next().await.ok_or_else(|| {
            anyhow!(
                "expected {} message following identification",
                stringify!($message_type)
            )
        })?;
        match next? {
            Message::Text(s) => match serde_json::from_str(&s)? {
                Event::$message_type($user_pat) => $result,
                e => bail!(
                    "first message received was not a {} message, got discriminant {:?}",
                    stringify!($message_type),
                    std::mem::discriminant(&e)
                ),
            },
            m => bail!(m),
        }
    }};
}

#[derive(Deserialize)]
struct BotGateway {
    url: String,
}

pub struct Bot {
    client: Client,
    auth: TokenBuf,
    intents: Intents,
}

impl Bot {
    pub fn new(auth: TokenBuf, intents: Intents) -> Self {
        Bot {
            client: Client::new(auth.as_ref()),
            auth,
            intents,
        }
    }

    async fn connect_to_gateway(&self) -> Result<WebSocket> {
        const GATEWAY_VERSION: &str = "8";
        const WSS_PORT: u16 = 443;
        let gateway_url = self
            .client
            .make_get_request::<BotGateway>("gateway/bot")
            .await?
            .url;

        let gateway_request = Url::parse_with_params(
            &gateway_url,
            &[("v", GATEWAY_VERSION), ("encoding", "json")],
        )?;

        let stream = Async::new(TcpStream::connect((
            gateway_request
                .host_str()
                .expect("Url must contain host name"),
            WSS_PORT,
        ))?)?;

        Ok(
            async_tungstenite::async_tls::client_async_tls(gateway_request, stream)
                .await
                .map(|(ws, _)| ws)?,
        )
    }

    async fn opening_handshake(
        &self,
        ws: &mut WebSocket,
        mut handler: impl AsyncDispatchHandler,
    ) -> Result<State> {
        send(
            ws,
            Identify {
                token: self.auth.clone(),
                properties: ConnectionProperties {
                    os: "windows".to_string(),
                    browser: "tungstenite".to_string(),
                    device: "rust".to_string(),
                },
                intents: self.intents,
                compress: None,
                large_threshold: None,
            },
        )
        .await?;

        let heartbeat_interval = expect_message_or_bail!(ws, h = Hello => h.heartbeat_interval);
        let (seq, session_id) = expect_message_or_bail!(ws,
        d = Dispatch => match &d.payload {
            DispatchPayload::Ready(ready) => {
                let r = (d.seq, ready.session_id.into_owned());
                handler.handle_message(d.payload, &self.client).await?;
                r
            },
            p => bail!(
                "dispatch payload expected to be Ready, got discriminant {:?}",
                std::mem::discriminant(&p)
            ),
        });

        Ok(State {
            seq,
            heartbeat_interval,
            session_id,
            heartbeat_acked: true,
        })
    }

    async fn reconnect(&self, ws: &mut WebSocket, state: &State) -> Result<()> {
        *ws = self.connect_to_gateway().await?;
        send(
            ws,
            Resume {
                token: self.auth.clone(),
                session_id: state.session_id.clone(),
                seq: state.seq,
            },
        )
        .await
    }

    async fn disconnect(&self, ws: &mut WebSocket) -> Result<()> {
        ws.close(None).await?;
        Ok(())
    }

    async fn handle_message(
        &self,
        ws: &mut WebSocket,
        state: &mut State,
        message: Message,
        mut handler: impl AsyncDispatchHandler,
    ) -> Result<()> {
        if let Message::Text(s) = &message {
            println!("{}", s);
            match serde_json::from_str::<Event>(s) {
                Ok(Event::Dispatch(d)) => {
                    if state.seq.0 + 1 != d.seq.0 {
                        eprintln!("Sequence gap: previous = {} got = {}", state.seq.0, d.seq.0);
                    }
                    state.seq = d.seq;
                    if let Err(e) = handler.handle_message(d.payload, &self.client).await {
                        eprintln!("{}", e);
                    }
                }
                Ok(Event::HeartbeatAck) => {
                    println!("heartbeat acknowledged");
                    state.heartbeat_acked = true;
                }
                Ok(Event::Reconnect) => {
                    println!("disconnecting (reconnect received)");
                    self.disconnect(ws).await?;
                }
                Err(e) => eprintln!("{}", e),
                _ => (),
            }
        }
        Ok(())
    }

    async fn run_loop(
        &self,
        ws: &mut WebSocket,
        mut state: State,
        mut handler: impl AsyncDispatchHandler,
    ) -> Result<()> {
        let mut timer = wait(state.heartbeat_interval);
        loop {
            let mut ws_fut = ws.next().fuse();
            select! {
                _ = timer => {
                    if !state.heartbeat_acked {
                        println!("disconnecting (heartbeat ack missed)");
                        self.disconnect(ws).await?;
                    } else {
                        send(ws, Heartbeat(Some(state.seq))).await?;
                        state.heartbeat_acked = false;
                        timer = wait(state.heartbeat_interval);
                    }
                }
                next = ws_fut => {
                    match next {
                        Some(msg) => self.handle_message(ws, &mut state, msg?, &mut handler).await?,
                        None => {
                            self.reconnect(ws, &state).await?;
                            timer = wait(state.heartbeat_interval);
                        }
                    }
                }
            }
        }
    }

    pub fn run(&self, mut handler: impl AsyncDispatchHandler) -> Result<()> {
        async_io::block_on(async move {
            let mut ws = self.connect_to_gateway().await?;
            let state = dbg!(self.opening_handshake(&mut ws, &mut handler).await?);
            self.run_loop(&mut ws, state, handler).await
        })
    }
}

pub type AsyncDispatchFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + 'a>>;

pub trait AsyncDispatchHandler {
    fn handle_message<'a>(
        &'a mut self,
        payload: DispatchPayload<'a>,
        client: &'a Client,
    ) -> AsyncDispatchFuture<'a>;
}

impl<T: AsyncDispatchHandler> AsyncDispatchHandler for &'_ mut T {
    fn handle_message<'a>(
        &'a mut self,
        payload: DispatchPayload<'a>,
        client: &'a Client,
    ) -> AsyncDispatchFuture<'a> {
        T::handle_message(*self, payload, client)
    }
}

#[derive(Debug)]
struct State {
    seq: Sequence,
    heartbeat_interval: u64,
    session_id: IdBuf,
    heartbeat_acked: bool,
}

fn wait(duration_millis: u64) -> impl FusedFuture {
    Timer::after(Duration::from_millis(duration_millis)).fuse()
}

async fn send(stream: &mut WebSocket, command: impl Command) -> Result<()> {
    stream
        .send(Message::Text(
            serde_json::to_string(&CommandSerializer(command)).expect("Command serialization"),
        ))
        .await?;
    Ok(())
}
