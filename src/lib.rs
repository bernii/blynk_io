//! # blynk_io
//!
//! `blynk_io` is a naive implementation of Blynk.io protocol
//! with intention of use in embedded systems. The intention is to use it
//! with esp32 devices in conjuction with esp-rs project.
//!
//! The `rust` implementation has been based on the official
//! python client implemetation since good blynk.io API docs are not avaiable.
//!
//! # Example usage
//!
//! ```ignore
//! use blynk_io::*;
//! ...
//! let mut blynk = <Blynk>::new("AUTH_TOKEN".to_string());
//!
//! fn main() {
//!    loop {
//!        blynk.run();
//!        thread::sleep(Duration::from_millis(50));
//!    }
//! }
//! ```
//!

use std::error::Error;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use std::thread;

use log::*;

mod client;
mod config;
mod message;

pub use self::client::{Client, Protocol};
pub use self::config::Config;
use self::message::{Message, MessageType, ProtocolStatus};

/// Represents the current state of connection to Blynk servers
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Authentiacting,
    Authenticated,
}

impl Default for ConnectionState {
    fn default() -> Self {
        ConnectionState::Disconnected
    }
}

/// Various defaults, mostly around connection timeouts and retry logic
mod conf {
    use std::time::Duration;

    pub const SOCK_MAX_TIMEOUT: Duration = Duration::from_secs(5);
    pub const SOCK_TIMEOUT: Duration = Duration::from_millis(1000);
    // const SOCK_SSL_TIMEOUT: u8 = 1; TODO: implement if SSL is neeeded
    pub const RETRIES_TX_DELAY: Duration = Duration::from_millis(2);
    pub const RETRIES_TX_MAX_NUM: u8 = 3;
    pub const RECONNECT_SLEEP: Duration = Duration::from_secs(1);
    pub const HEARTBEAT_PERIOD: Duration = Duration::from_secs(5);
}

/// Used in order to implement handler logic for requests coming
/// from Blynk.io servers and various transitions between connection states.
///
/// # Example
/// ```
/// use blynk_io::*;
///
/// struct EventsHandler;
/// impl Event for EventsHandler {
///     fn handle_vpin_write(&mut self, _client: &mut Client, pin_num: u8, data: &str) {
///         println!("pin {:?} write {:?}", pin_num, data);
///     }
/// }
/// ```
#[allow(unused_variables)]
pub trait Event: Send {
    fn handle_connect(&mut self, client: &mut Client) {}
    fn handle_disconnect(&mut self) {}
    fn handle_internal(&mut self, client: &mut Client, data: &[String]) {}
    fn handle_vpin_read(&mut self, client: &mut Client, pin_num: u8) {}
    fn handle_vpin_write(&mut self, client: &mut Client, pin_num: u8, data: &str) {}
}

/// Default events handler implementation that can be used
/// to define type if no client implementation is provided
pub struct DefaultHandler {}
impl Event for DefaultHandler {}

use std::result;
use std::{fmt, io};

#[derive(Debug)]
pub enum BlynkError {
    Io(io::Error),
    Dns,
    MessageSend,
    EmptyBuffer,
    Redirection,
    HeartbeatSet(ProtocolStatus),
    InvalidAuthToken,
    InvalidMessageId,
    InvalidMessageHeader,
    InvalidMessageBody,
    StreamIsNone,
    ReaderNotAvailable,
}

impl fmt::Display for BlynkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            BlynkError::Io(ref err) => err.fmt(f),
            BlynkError::Dns => write!(f, "Problem resolving host"),
            BlynkError::MessageSend => write!(f, "Problem sending message"),
            BlynkError::EmptyBuffer => write!(f, "No message to process"),
            BlynkError::Redirection => write!(f, "Redirection problem"),
            BlynkError::HeartbeatSet(ref ps) => write!(f, "Problem setting heartbeat {:?}", ps),
            BlynkError::InvalidAuthToken => write!(f, "Invalid auth token"),
            BlynkError::InvalidMessageId => write!(f, "Message id is zero"),
            BlynkError::InvalidMessageHeader => write!(f, "Problem parsing message header"),
            BlynkError::InvalidMessageBody => write!(f, "Malformed message body"),
            BlynkError::StreamIsNone => write!(f, "Stream not available"),
            BlynkError::ReaderNotAvailable => write!(f, "Unable to access reader"),
        }
    }
}

impl Error for BlynkError {}

impl From<io::Error> for BlynkError {
    fn from(err: io::Error) -> BlynkError {
        BlynkError::Io(err)
    }
}

type Result<T> = result::Result<T, BlynkError>;

/// Main API for interacting with Blynk.io platform. Use it in order to
/// keep connectivity with the Blynk servers and handle the protocol activity.
///
/// # Example
/// ```
/// use blynk_io::Blynk;
///
/// let mut blynk = <Blynk>::new("BYNK TOKEN".to_string());
/// loop {
///     blynk.run();
///     break; // remove this in your actual program
/// }
/// ```

pub struct Blynk<E: Event = DefaultHandler> {
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
    fn client(&mut self) -> &mut Client {
        self.last_send_time = Instant::now();
        &mut self.client
    }

    /// Performs event loop run that is reposnible for:
    /// - checking the connection state
    /// - reconnecting if connection failed
    /// - reading any pending responses from blynk servers
    /// - executing events hooks if those are provided
    pub fn run(&mut self) {
        if !matches!(self.conn_state, ConnectionState::Authenticated) {
            error!("Not connected, trying reconnect");
            if let Err(err) = self.connect() {
                error!("Problem while connecting: {}", err);
                self.disconnect("Problem while connecting");
                return;
            }
        }

        self.read_response();
        if !self.is_server_alive() {
            info!("Blynk is offline for some reson :(");
            self.disconnect("Blynk server is offline");
        }
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
    fn connect(&mut self) -> Result<()> {
        self.conn_state = ConnectionState::Connecting;

        let host_port = vec![
            self.config.server.clone(),
            ":".to_string(),
            self.config.port.to_string(),
        ]
        .join("");
        let addrs = host_port.to_socket_addrs()?.collect::<Vec<_>>();
        let addr = addrs.first().ok_or(BlynkError::Dns)?;

        let stream = TcpStream::connect_timeout(addr, conf::SOCK_TIMEOUT)?;
        self.client.set_stream(stream);

        info!("Successfully connected to blynk server");

        self.authenticate(&self.config.token.clone())?;
        self.set_heartbeat()?;

        self.last_rcv_time = Instant::now();

        if let Some(hook) = &mut self.handler {
            hook.handle_connect(&mut self.client);
        }
        Ok(())
    }

    /// Disconnects from the Blynk servers
    ///
    /// Calls disconnect hook
    fn disconnect(&mut self, msg: &str) {
        if let Some(hook) = &mut self.handler {
            hook.handle_disconnect();
        }

        self.client.disconnect();
        self.conn_state = ConnectionState::Disconnected;
        error!("{}", msg);

        thread::sleep(conf::RECONNECT_SLEEP);
    }

    fn authenticate(&mut self, token: &str) -> Result<()> {
        info!("Authenticating device...");
        self.conn_state = ConnectionState::Authentiacting;
        self.client().login(token)?;

        let msg = self.client.read().unwrap();
        if !matches!(msg.status, Some(ProtocolStatus::StatusOk)) {
            match (msg.status.unwrap(), msg.mtype) {
                (ProtocolStatus::StatusInvalidToken, _) => {
                    return Err(BlynkError::InvalidAuthToken);
                }
                (_, MessageType::Redirect) => {
                    return Err(BlynkError::Redirection);
                }
                (_, _) => panic!("Critical error"),
            }
        }

        self.conn_state = ConnectionState::Authenticated;
        info!("Access granted");
        Ok(())
    }

    fn set_heartbeat(&mut self) -> Result<()> {
        info!("Setting heartbeat");
        self.client().heartbeat(conf::HEARTBEAT_PERIOD, 1024)?;

        self.client.set_read_timeout(conf::SOCK_MAX_TIMEOUT);
        let msg = self.client.read()?;

        if !matches!(msg.status, Some(ProtocolStatus::StatusOk)) {
            return Err(BlynkError::HeartbeatSet(msg.status.unwrap()));
        }
        Ok(())
    }

    fn is_server_alive(&mut self) -> bool {
        let hbeat_ms = conf::HEARTBEAT_PERIOD.as_millis();
        let rcv_delta = self.last_rcv_time.elapsed().as_millis();
        let ping_delta = self.last_ping_time.elapsed().as_millis();
        let send_delta = self.last_send_time.elapsed().as_millis();

        if rcv_delta > hbeat_ms + (hbeat_ms / 2) {
            warn!("Server not alive, will initiate disconnect");
            return false;
        }

        if (ping_delta > hbeat_ms / 10) && (send_delta > hbeat_ms || rcv_delta > hbeat_ms) {
            if self.client().ping().is_err() {
                error!("Unable to ping");
                return false;
            }

            self.last_ping_time = Instant::now();
            info!("Heartbeat delta: {}ms", ping_delta);
        }

        true
    }

    fn read_response(&mut self) {
        self.last_rcv_time = Instant::now();
        self.client.set_read_timeout(Duration::from_millis(5));

        if let Ok(msg) = self.client.read() {
            if let Err(err) = self.process(msg) {
                error!("Problem handling req from API: {}", err);
            }
        }
    }

    fn process(&mut self, msg: Message) -> Result<()> {
        if let MessageType::Ping = msg.mtype {
            self.client
                .response(ProtocolStatus::StatusOk as u16, msg.id)?;
        }

        if let Some(hook) = &mut self.handler {
            match msg.mtype {
                MessageType::Internal => {
                    hook.handle_internal(&mut self.client, &msg.body[1..]);
                }
                MessageType::Hw | MessageType::Bridge => {
                    if msg.body.len() >= 3 && msg.body.get(0).unwrap() == "vw" {
                        let pin_num = msg.body[1].parse::<u8>().unwrap();
                        hook.handle_vpin_write(&mut self.client, pin_num, &msg.body[2]);
                    } else if msg.body.len() == 2 && msg.body.get(0).unwrap() == "vr" {
                        let pin_num = msg.body[1].parse::<u8>().unwrap();
                        hook.handle_vpin_read(&mut self.client, pin_num);
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct EventsHandler {
        pin_num: u8,
        data: String,
    }

    impl Event for EventsHandler {
        fn handle_vpin_read(&mut self, _client: &mut Client, pin_num: u8) {
            self.pin_num = pin_num
        }

        fn handle_vpin_write(&mut self, _client: &mut Client, pin_num: u8, data: &str) {
            self.pin_num = pin_num;
            self.data = data.to_string();
        }

        fn handle_internal(&mut self, _client: &mut Client, data: &[String]) {
            self.data = data.join(" ");
        }
    }

    #[test]
    fn calls_vpinread_handler_with_params() {
        let msg = Message::new(MessageType::Hw, 1, None, None, vec!["vr", "22"]);
        let mut blynk = Blynk::new("abc".to_string());

        let handler: EventsHandler = Default::default();
        blynk.set_handler(handler);
        blynk.process(msg).unwrap();

        assert_eq!(22, blynk.handler().unwrap().pin_num);
    }
    #[test]
    fn calls_vpinwrite_handler_with_params() {
        let msg = Message::new(MessageType::Hw, 1, None, None, vec!["vw", "42", "my-val"]);
        let mut blynk = Blynk::new("abc".to_string());

        let handler: EventsHandler = Default::default();
        blynk.set_handler(handler);
        blynk.process(msg).unwrap();

        assert_eq!(42, blynk.handler().unwrap().pin_num);
        assert_eq!("my-val", blynk.handler().unwrap().data);
    }
    #[test]
    fn calls_internal_handler_with_params() {
        let msg = Message::new(
            MessageType::Internal,
            1,
            None,
            None,
            vec!["_internal", "hello", "world"],
        );
        let mut blynk = Blynk::new("abc".to_string());

        let handler: EventsHandler = Default::default();
        blynk.set_handler(handler);
        blynk.process(msg).unwrap();

        assert_eq!("hello world", blynk.handler().unwrap().data);
    }
}
