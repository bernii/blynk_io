//! # Blynk.io Library
//!
//! `blynk_io` is naive implementation of Blynk.io protocol
//! with intention of use in embedded systems. Tested mainly with esp32
//!

use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use std::error::Error;
use std::thread;

use log::*;

mod client;
mod config;
mod message;

pub use self::client::Client;
pub use self::config::Config;
use self::message::{Message, MessageType, ProtocolStatus};

pub enum ConnectionState {
    Disconnected,
    Connecting,
    Authentiacting,
    Authenticated,
}

mod conf {
    use std::time::Duration;

    pub const SOCK_MAX_TIMEOUT: Duration = Duration::from_secs(5);
    pub const SOCK_TIMEOUT: Duration = Duration::from_millis(15000);
    // const SOCK_SSL_TIMEOUT: u8 = 1; TODO: implement if SSL is neeeded
    pub const RETRIES_TX_DELAY: Duration = Duration::from_millis(2);
    pub const RETRIES_TX_MAX_NUM: u8 = 3;
    pub const RECONNECT_SLEEP: Duration = Duration::from_secs(1);
    pub const READ_TIMEOUT: Duration = Duration::from_millis(500);
    pub const HEARTBEAT_PERIOD: Duration = Duration::from_secs(5);
}

#[allow(unused_variables)]
pub trait Event {
    fn handle_connect(&mut self, client: &mut Client) {}
    fn handle_disconnect(&mut self) {}
    fn handle_internal(&mut self, client: &mut Client) {}
    fn handle_vpin_read(&mut self, client: &mut Client, pin_num: u8) {}
    fn handle_vpin_write(&mut self, client: &mut Client, pin_num: u8, data: &str) {}
}

pub struct Blynk {
    conn_state: ConnectionState,
    auth_token: String,

    client: Client,

    events_hook: Option<Box<dyn Event>>,

    last_rcv_time: Instant,
    last_ping_time: Instant,
    last_send_time: Instant,
}

impl Blynk {
    pub fn new(auth_token: String) -> Blynk {
        Blynk {
            conn_state: ConnectionState::Disconnected,
            auth_token,

            client: Client::default(),
            events_hook: None,

            last_rcv_time: Instant::now(),
            last_ping_time: Instant::now(),
            last_send_time: Instant::now(),
        }
    }

    pub fn client(&mut self) -> &mut Client {
        self.last_send_time = Instant::now();
        &mut self.client
    }

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
            self.disconnect("Blynk server is offline");
        }
    }

    pub fn set_events_hook<E: Event + 'static>(&mut self, hook: E) {
        let hook = Box::new(hook);
        self.events_hook = Some(hook);
    }

    fn connect(&mut self) -> Result<(), Box<dyn Error>> {
        self.conn_state = ConnectionState::Connecting;

        let addrs = "blynk-cloud.com:80".to_socket_addrs()?.collect::<Vec<_>>();
        let addr = addrs.first().ok_or("Problem resolving server addr")?;

        let stream = TcpStream::connect_timeout(addr, conf::SOCK_TIMEOUT)?;
        self.client.set_stream(stream);

        info!("Successfully connected to blynk server");

        self.authenticate(&self.auth_token.clone())?;
        self.set_heartbeat()?;

        self.last_rcv_time = Instant::now();

        if let Some(hook) = &mut self.events_hook {
            hook.handle_connect(&mut self.client);
        }
        Ok(())
    }

    pub fn disconnect(&mut self, msg: &str) {
        if let Some(hook) = &mut self.events_hook {
            hook.handle_disconnect();
        }

        self.client.disconnect();
        self.conn_state = ConnectionState::Disconnected;
        error!("{}", msg);

        thread::sleep(conf::RECONNECT_SLEEP);
    }

    fn authenticate(&mut self, token: &str) -> Result<(), Box<dyn Error>> {
        info!("Authenticating device...");
        self.conn_state = ConnectionState::Authentiacting;
        self.client().login(token)?;

        let msg = self.client.read().unwrap();
        if !matches!(msg.status, Some(ProtocolStatus::StatusOk)) {
            match (msg.status.unwrap(), msg.mtype) {
                (ProtocolStatus::StatusInvalidToken, _) => {
                    return Err("Invalid auth token".into());
                }
                (_, MessageType::Redirect) => {
                    return Err("Redirection problem".into());
                }
                (_, _) => panic!("Critical error"),
            }
        }

        self.conn_state = ConnectionState::Authenticated;
        info!("Access granted");
        Ok(())
    }

    fn set_heartbeat(&mut self) -> Result<(), Box<dyn Error>> {
        info!("Setting heartbeat");
        self.client().heartbeat(conf::HEARTBEAT_PERIOD, 1024)?;

        self.client.set_read_timeout(conf::SOCK_MAX_TIMEOUT);
        let msg = self.client.read()?;

        if !matches!(msg.status, Some(ProtocolStatus::StatusOk)) {
            return Err(format!("Problem setting heartbeat {:?}", msg.status.unwrap()).into());
        }
        Ok(())
    }

    pub fn is_server_alive(&mut self) -> bool {
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
            info!("Heartbeat delta: {}", ping_delta);
        }

        true
    }

    pub fn read_response(&mut self) {
        let start = Instant::now();
        while start.elapsed() <= conf::READ_TIMEOUT {
            self.last_rcv_time = Instant::now();
            self.client.set_read_timeout(Duration::from_millis(5));

            if let Ok(msg) = self.client.read() {
                // TODO: add error handling
                self.process(msg);
            }
        }
    }

    fn process(&mut self, msg: Message) -> Result<(), Box<dyn Error>> {
        if let MessageType::Ping = msg.mtype {
            self.client
                .response(ProtocolStatus::StatusOk as u16, msg.id)?;
        }

        if let Some(hook) = &mut self.events_hook {
            match msg.mtype {
                MessageType::Internal => {
                    // TODO XXX
                    // self.call_handler("{}{}".format(self._INTERNAL, msg_args[0]), msg_args[1:])
                    hook.handle_internal(&mut self.client);
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
    #[test]
    fn calls_handler_if_hook_provided() {}
    #[test]
    fn calls_vpinwrite_handler_with_params() {}
    #[test]
    fn calls_vpinread_handler_with_params() {}
    #[test]
    fn calls_internal_handler_with_params() {}
}
