use log::*;

pub use self::client::{Client, Protocol};

pub mod client;

use crate::message::Message;
use crate::{BlynkError, Config, ConnectionState, DefaultHandler, Result};
use async_trait::async_trait;

use crate::conf;
use crate::message::{MessageType, ProtocolStatus};

use smol::future::FutureExt;
use smol::{Async, Timer};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

#[allow(unused_variables)]
#[async_trait]
pub trait Event: Send {
    async fn handle_connect(&mut self, client: &mut Client) {}
    async fn handle_disconnect(&mut self) {}
    async fn handle_internal(&mut self, client: &mut Client, data: &[String]) {}
    async fn handle_vpin_read(&mut self, client: &mut Client, pin_num: u8) {}
    async fn handle_vpin_write(&mut self, client: &mut Client, pin_num: u8, data: &str) {}
}

#[async_trait]
impl Event for DefaultHandler {}

pub struct Blynk<E: Event> {
    conn_state: ConnectionState,
    config: Config,

    client: Client,

    pub handler: Option<E>,

    last_rcv_time: Instant,
    last_ping_time: Instant,
    last_send_time: Instant,
}

impl<E: Event> Blynk<E> {
    /// Returns the Blynk client initalized with API token
    ///
    /// # Arguments
    /// * `auth_token` - A string that holds the Blynk API token
    pub fn new(auth_token: String) -> Blynk<E> {
        Self {
            conn_state: ConnectionState::Disconnected,
            config: Config {
                token: auth_token,
                ..Default::default()
            },

            client: Client::default(),
            handler: None,

            last_rcv_time: Instant::now(),
            last_ping_time: Instant::now(),
            last_send_time: Instant::now(),
        }
    }

    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    /// Returns the low level Client abstraction that is implements
    /// the protocol and is responsible for the communication
    pub fn client(&mut self) -> &mut Client {
        self.last_send_time = Instant::now();
        &mut self.client
    }

    /// Performs event loop run that is reposnible for:
    /// - checking the connection state
    /// - reconnecting if connection failed
    /// - reading any pending responses from blynk servers
    /// - executing events hooks if those are provided
    pub async fn run(&mut self) {
        if !matches!(self.conn_state, ConnectionState::Authenticated) {
            error!("Not connected, trying reconnect");
            if let Err(err) = self.connect().await {
                error!("Problem while connecting: {}", err);
                self.disconnect("Problem while connecting").await;
                return;
            }
        }

        if !self.is_server_alive().await {
            info!("Blynk is offline for some reson :(");
            self.disconnect("Blynk server is offline").await;
            return;
        }

        // otherwise wait for response
        self.read_response()
            .or(async {
                Timer::after(Duration::from_millis(5)).await;
            })
            .await;
    }

    /// Sets the events handler for incoming events from the Blynk platform
    ///
    /// See `Event` trait documentation for example implementation
    pub fn set_handler(&mut self, hook: E) {
        self.handler = Some(hook);
    }

    /// Gets a mutable referance to handler if it's defined
    pub fn handler(&mut self) -> Option<&mut E> {
        match &self.handler {
            Some(_) => self.handler.as_mut(),
            None => None,
        }
    }

    /// Connects to Blynk servers
    ///
    /// Performs authentication and sets up heart beat with the servers
    ///
    /// Calls hook in event of succseful handshake
    async fn connect(&mut self) -> Result<()> {
        self.conn_state = ConnectionState::Connecting;

        let host_port = vec![
            self.config.server.clone(),
            ":".to_string(),
            self.config.port.to_string(),
        ]
        .join("");

        let addrs = smol::unblock(move || host_port.to_socket_addrs())
            .await?
            .collect::<Vec<_>>();
        let addr = *addrs.first().ok_or(BlynkError::Dns)?;

        info!("stream open start tp {:?}", addr);

        // opening async TcpStream connection does not work yet with esp-rs
        let blocking_stream =
            smol::unblock(move || TcpStream::connect_timeout(&addr, Duration::from_secs(3)))
                .await?;
        let stream = Async::new(blocking_stream)?;

        // once it works ;-)
        // let stream = Async::<TcpStream>::connect(addr).or(async {
        //     Timer::after(Duration::from_secs(10)).await;
        //     Err(io::ErrorKind::TimedOut.into())
        // })
        // .await.unwrap();

        self.client.set_stream(stream);

        info!("Successfully connected to blynk server");

        self.authenticate(&self.config.token.clone()).await?;
        self.set_heartbeat().await?;

        self.last_rcv_time = Instant::now();

        if let Some(hook) = &mut self.handler {
            hook.handle_connect(&mut self.client).await;
        }
        Ok(())
    }

    /// Disconnects from the Blynk servers
    ///
    /// Calls disconnect hook
    async fn disconnect(&mut self, msg: &str) {
        if let Some(hook) = &mut self.handler {
            hook.handle_disconnect().await;
        }

        self.client.disconnect();
        self.conn_state = ConnectionState::Disconnected;
        error!("{}", msg);

        // thread::sleep(conf::RECONNECT_SLEEP);
        info!("1s sleep start");
        smol::Timer::after(conf::RECONNECT_SLEEP).await;
    }

    async fn authenticate(&mut self, token: &str) -> Result<()> {
        info!("Authenticating device...");
        self.conn_state = ConnectionState::Authentiacting;
        self.client().login(token).await?;

        let msg = self.client.read().await.unwrap();
        if !matches!(msg.status, Some(ProtocolStatus::StatusOk)) {
            match (msg.status.unwrap(), msg.mtype) {
                (ProtocolStatus::StatusInvalidToken, _) => {
                    return Err(BlynkError::InvalidAuthToken.into());
                }
                (_, MessageType::Redirect) => {
                    return Err(BlynkError::Redirection.into());
                }
                (_, _) => panic!("Critical error"),
            }
        }

        self.conn_state = ConnectionState::Authenticated;
        info!("Access granted");
        Ok(())
    }

    async fn set_heartbeat(&mut self) -> Result<()> {
        info!("Setting heartbeat");
        self.client()
            .heartbeat(conf::HEARTBEAT_PERIOD, 1024)
            .await?;

        self.client.set_read_timeout(conf::SOCK_MAX_TIMEOUT);
        let msg = self.client.read().await?;

        if !matches!(msg.status, Some(ProtocolStatus::StatusOk)) {
            return Err(BlynkError::HeartbeatSet(msg.status.unwrap()).into());
        }
        Ok(())
    }

    async fn is_server_alive(&mut self) -> bool {
        let hbeat_ms = conf::HEARTBEAT_PERIOD.as_millis();
        let rcv_delta = self.last_rcv_time.elapsed().as_millis();
        let ping_delta = self.last_ping_time.elapsed().as_millis();
        let send_delta = self.last_send_time.elapsed().as_millis();

        if rcv_delta > hbeat_ms + (hbeat_ms / 2) {
            warn!("Server not alive, will initiate disconnect");
            return false;
        }

        if (ping_delta > hbeat_ms / 10) && (send_delta > hbeat_ms || rcv_delta > hbeat_ms) {
            if self.client().ping().await.is_err() {
                error!("Unable to ping");
                return false;
            }

            self.last_ping_time = Instant::now();
            info!("Heartbeat delta: {}ms", ping_delta);
        }

        true
    }

    async fn read_response(&mut self) {
        self.last_rcv_time = Instant::now();
        self.client.set_read_timeout(Duration::from_millis(5));

        if let Ok(msg) = self.client.read().await {
            if let Err(err) = self.process(msg).await {
                error!("Problem handling req from API: {}", err);
            }
        }
    }

    async fn process(&mut self, msg: Message) -> Result<()> {
        info!("Message processing ASD {:?}", msg);
        if let MessageType::Ping = msg.mtype {
            self.client
                .response(ProtocolStatus::StatusOk as u16, msg.id)
                .await?;
        }

        if let Some(hook) = &mut self.handler {
            match msg.mtype {
                MessageType::Internal => {
                    hook.handle_internal(&mut self.client, &msg.body[1..]).await;
                }
                MessageType::Hw | MessageType::Bridge => {
                    if msg.body.len() >= 3 && msg.body.get(0).unwrap() == "vw" {
                        let pin_num = msg.body[1].parse::<u8>().unwrap();
                        hook.handle_vpin_write(&mut self.client, pin_num, &msg.body[2])
                            .await;
                    } else if msg.body.len() == 2 && msg.body.get(0).unwrap() == "vr" {
                        let pin_num = msg.body[1].parse::<u8>().unwrap();
                        hook.handle_vpin_read(&mut self.client, pin_num).await;
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }
}
