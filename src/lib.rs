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

mod config;
mod message;

#[cfg(feature = "async")]
mod async_impl;
#[cfg(feature = "async")]
pub use self::async_impl::{Blynk, Client, Event, Protocol};

#[cfg(not(feature = "async"))]
mod blocking;
#[cfg(not(feature = "async"))]
pub use self::blocking::{Blynk, Client, Event, Protocol};

pub use self::config::Config;

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

/// Default events handler implementation that can be used
/// to define type if no client implementation is provided
pub struct DefaultHandler {}

use std::result;
use std::{fmt, io};

#[derive(Debug)]
pub enum BlynkError {
    Io(io::Error),
    Dns,
    MessageSend,
    EmptyBuffer,
    Redirection,
    HeartbeatSet(message::ProtocolStatus),
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
