use std::fmt;
use std::io::prelude::*;
use std::io::BufReader;
use std::net::{Shutdown, TcpStream};
use std::thread;
use std::time::Duration;

use log::*;

use crate::conf;
use crate::message::{Message, MessageType, ProtocolHeader};
use crate::{BlynkError, Result};

const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
struct ClientError;

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "client encountered an error")
    }
}

// impl Error for ClientError {
//     fn description(&self) -> &str {
//         "client error lol"
//     }
// }

#[derive(Default)]
/// Implements state of the connection abstraction with Blynk.io servers.
/// Implementes protocol methods that you can use in order to
/// communicate with those servers
pub struct Client {
    msg_id: u16,
    reader: Option<BufReader<TcpStream>>,
}

impl Client {
    pub fn set_read_timeout(&mut self, duration: Duration) {
        if let Ok(stream) = self.stream() {
            stream
                .set_read_timeout(Some(duration))
                .expect("read timeout problem");
        }
    }
}

/// Provides implementation of all known blynk.io api protocol methods
pub trait Protocol {
    type T: std::io::Read + std::io::Write;

    fn set_reader(&mut self, reader: BufReader<Self::T>);
    fn msg_id(&mut self) -> u16;
    fn disconnect(&mut self);
    fn reader(&mut self) -> Option<&mut BufReader<Self::T>>;

    fn set_stream(&mut self, stream: Self::T) {
        self.set_reader(BufReader::new(stream));
    }

    fn read(&mut self) -> Result<Message> {
        let reader = self.reader().ok_or(BlynkError::ReaderNotAvailable)?;

        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            return Err(BlynkError::EmptyBuffer);
        }
        let msg = Message::deserilize(buf)?;

        debug!(
            "size ({}) vs consumed ({})",
            buf.len(),
            ProtocolHeader::SIZE + msg.size.unwrap_or(0) as usize
        );

        // consume bytes (msg header + body) from the reader
        reader.consume(ProtocolHeader::SIZE + msg.size.unwrap_or(0) as usize);
        debug!("Got response message: {:?}", msg);
        Ok(msg)
    }

    fn stream(&mut self) -> Result<&mut Self::T> {
        if let Some(r) = self.reader() {
            return Ok(r.get_mut());
        }
        Err(BlynkError::StreamIsNone)
    }

    fn login(&mut self, token: &str) -> Result<()> {
        let msg = Message::new(MessageType::Login, self.msg_id(), None, None, vec![token]);
        self.send(msg.serialize())
    }

    fn heartbeat(&mut self, heartbeat: Duration, rcv_buffer: u16) -> Result<()> {
        let msg = Message::new(
            MessageType::Internal,
            self.msg_id(),
            None,
            None,
            vec![
                "ver",
                CARGO_PKG_VERSION,
                "buff-in",
                &rcv_buffer.to_string(),
                "h-beat",
                &heartbeat.as_secs().to_string(),
                "dev",
                "rust",
            ],
        );

        self.send(msg.serialize())
    }

    fn ping(&mut self) -> Result<()> {
        let msg = Message::new(MessageType::Ping, self.msg_id(), None, None, vec![]);
        self.send(msg.serialize())
    }

    fn response(&mut self, status: u16, msg_id: u16) -> Result<()> {
        let msg = Message::new(
            MessageType::Rsp,
            msg_id,
            None,
            None,
            vec![&status.to_string()],
        );
        self.send(msg.serialize())
    }

    fn virtual_write(&mut self, v_pin: u8, val: &str) -> Result<()> {
        let msg = Message::new(
            MessageType::Hw,
            self.msg_id(),
            None,
            None,
            vec!["vw", &v_pin.to_string(), val],
        );
        self.send(msg.serialize())
    }

    fn virtual_sync(&mut self, pins: Vec<u32>) -> Result<()> {
        let pins: String = pins
            .into_iter()
            .map(|x| std::char::from_digit(x, 10).unwrap())
            .collect();

        let msg = Message::new(
            MessageType::HwSync,
            self.msg_id(),
            None,
            None,
            vec!["vr", &pins],
        );
        self.send(msg.serialize())
    }

    fn email(&mut self, to: &str, subject: &str, body: &str) -> Result<()> {
        let msg = Message::new(
            MessageType::Email,
            self.msg_id(),
            None,
            None,
            vec![to, subject, body],
        );
        self.send(msg.serialize())
    }

    fn tweet(&mut self, msg: &str) -> Result<()> {
        let msg = Message::new(MessageType::Tweet, self.msg_id(), None, None, vec![msg]);
        self.send(msg.serialize())
    }

    fn notify(&mut self, msg: &str) -> Result<()> {
        let msg = Message::new(MessageType::Notify, self.msg_id(), None, None, vec![msg]);
        self.send(msg.serialize())
    }

    fn set_property(&mut self, pin: u8, prop: &str, val: &str) -> Result<()> {
        let msg = Message::new(
            MessageType::Property,
            self.msg_id(),
            None,
            None,
            vec![&pin.to_string(), prop, val],
        );
        self.send(msg.serialize())
    }

    fn internal(&mut self, data: Vec<&str>) -> Result<()> {
        let msg = Message::new(MessageType::Internal, self.msg_id(), None, None, data);
        self.send(msg.serialize())
    }

    fn send(&mut self, msg: Vec<u8>) -> Result<()> {
        let mut retries = conf::RETRIES_TX_MAX_NUM;
        let stream = self.stream()?;
        while retries > 0 {
            if let Err(err) = stream.write(&msg) {
                eprintln!("Problem sending!: {}", err);
                retries -= 1;
                thread::sleep(conf::RETRIES_TX_DELAY);
                continue;
            }
            if let Err(err) = stream.flush() {
                eprintln!("Problem sending!: {}", err);
                retries -= 1;
                thread::sleep(conf::RETRIES_TX_DELAY);
                continue;
            }
            debug!("Sent message, awaiting reply...!!");
            return Ok(());
        }
        Err(BlynkError::MessageSend)
    }
}

impl Protocol for Client {
    type T = TcpStream;

    fn set_reader(&mut self, reader: BufReader<TcpStream>) {
        self.reader = Some(reader);
    }

    fn reader(&mut self) -> Option<&mut BufReader<TcpStream>> {
        self.reader.as_mut()
    }

    fn msg_id(&mut self) -> u16 {
        self.msg_id += 1;
        self.msg_id
    }

    fn disconnect(&mut self) {
        if let Ok(stream) = self.stream() {
            stream
                .shutdown(Shutdown::Both)
                .unwrap_or_else(|err| error!("shutdown call failed, with err {}", err));
        }
        self.msg_id = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, SeekFrom};

    pub struct FakeClient {
        msg_id: u16,
        reader: Option<BufReader<Cursor<Vec<u8>>>>,
    }

    impl Protocol for FakeClient {
        type T = Cursor<Vec<u8>>;

        fn set_reader(&mut self, _reader: BufReader<Self::T>) {}

        fn reader(&mut self) -> Option<&mut BufReader<Self::T>> {
            return self.reader.as_mut();
        }

        fn msg_id(&mut self) -> u16 {
            self.msg_id += 1;
            self.msg_id
        }

        fn disconnect(&mut self) {
            self.msg_id = 0;
        }
    }

    #[test]
    fn msg_id_incremeneted_on_send() {
        let mut client = Client {
            msg_id: 3,
            reader: None,
        };
        client.ping().unwrap_or_default();
        assert_eq!(4, client.msg_id)
    }
    #[test]
    fn msg_id_customized() {
        let mut client = Client {
            msg_id: 3,
            reader: None,
        };
        client.response(200, 42).unwrap_or_default();
        // inspect the message
        assert_eq!(3, client.msg_id)
    }
    #[test]
    fn propagate_send_err() {
        let mut client = Client {
            msg_id: 3,
            reader: None,
        };
        assert!(client.ping().is_err());
    }
    #[test]
    fn ping_generates_seralized_message() {
        let reader = BufReader::with_capacity(10, Cursor::new(vec![0; 10]));
        let mut client = FakeClient {
            msg_id: 0,
            reader: Some(reader),
        };

        // intercept message into fake client
        client.ping().unwrap();

        let mut reader = client.reader.unwrap();
        reader.seek(SeekFrom::Start(0)).unwrap(); // rewind the buffer
        let buf = reader.fill_buf().unwrap();

        let msg = Message::new(MessageType::Ping, 1, None, None, vec![""]);
        let data = msg.serialize();
        // compare generated headers
        assert_eq!(&data[..5], &buf[..5]);
    }
    #[test]
    fn read_empty_buffer_errors() {
        // try to read when the buffer is empty
        let reader = BufReader::with_capacity(0, Cursor::new(vec![0]));
        let mut client = FakeClient {
            msg_id: 0,
            reader: Some(reader),
        };
        let err = client.read().err().unwrap();
        assert_eq!("No message to process", err.to_string());
    }
    #[test]
    fn read_message() {
        // succesful message read

        // put fake message into the buff
        let msg = Message::new(MessageType::Hw, 1, None, None, vec![""]);
        let reader = BufReader::with_capacity(10, Cursor::new(msg.serialize()));

        // intercept message into fake client
        let mut client = FakeClient {
            msg_id: 0,
            reader: Some(reader),
        };
        assert!(client.read().is_ok());
    }
}
