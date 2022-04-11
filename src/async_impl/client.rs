use std::net::TcpStream;
use std::thread;
use std::time::Duration;

use log::*;

use crate::conf;
use crate::message::{Message, MessageType, ProtocolHeader};
use crate::{BlynkError, Result};

const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

use smol::io::BufReader;
use smol::prelude::{AsyncRead, AsyncWrite};
use smol::Async;
#[derive(Default)]
/// Implements state of the connection abstraction with Blynk.io servers.
/// Implementes protocol methods that you can use in order to
/// communicate with those servers
pub struct Client {
    msg_id: u16,
    reader: Option<BufReader<Async<TcpStream>>>,
}

impl Client {
    pub fn set_read_timeout(&mut self, _duration: Duration) {
    }
}

/// Provides implementation of all known blynk.io api protocol methods
use async_trait::async_trait;
use smol::io::{AsyncBufReadExt, AsyncWriteExt, AsyncSeekExt};

#[async_trait]
pub trait Protocol {
    type T: AsyncRead + AsyncWrite + Unpin + Send;

    fn set_reader(&mut self, reader: BufReader<Self::T>);
    fn msg_id(&mut self) -> u16;
    fn disconnect(&mut self);
    fn reader(&mut self) -> Option<&mut BufReader<Self::T>>;

    fn set_stream(&mut self, stream: Self::T) {
        self.set_reader(BufReader::new(stream));
    }

    async fn read(&mut self) -> Result<Message> {
        let reader = self.reader().ok_or(BlynkError::ReaderNotAvailable)?;

        let buf = reader.fill_buf().await?;
        if buf.is_empty() {
            return Err(BlynkError::EmptyBuffer.into());
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
        Err(BlynkError::StreamIsNone.into())
    }

    async fn login(&mut self, token: &str) -> Result<()> {
        let msg = Message::new(MessageType::Login, self.msg_id(), None, None, vec![token]);
        self.send(msg.serialize()).await
    }

    async fn heartbeat(&mut self, heartbeat: Duration, rcv_buffer: u16) -> Result<()> {
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

        self.send(msg.serialize()).await
    }

    async fn ping(&mut self) -> Result<()> {
        let msg = Message::new(MessageType::Ping, self.msg_id(), None, None, vec![]);
        self.send(msg.serialize()).await
    }

    async fn response(&mut self, status: u16, msg_id: u16) -> Result<()> {
        let msg = Message::new(
            MessageType::Rsp,
            msg_id,
            None,
            None,
            vec![&status.to_string()],
        );
        self.send(msg.serialize()).await
    }

    async fn virtual_write(&mut self, v_pin: u8, val: &str) -> Result<()> {
        let msg = Message::new(
            MessageType::Hw,
            self.msg_id(),
            None,
            None,
            vec!["vw", &v_pin.to_string(), val],
        );
        self.send(msg.serialize()).await
    }

    async fn virtual_sync(&mut self, pins: Vec<u32>) -> Result<()> {
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
        self.send(msg.serialize()).await
    }

    async fn email(&mut self, to: &str, subject: &str, body: &str) -> Result<()> {
        let msg = Message::new(
            MessageType::Email,
            self.msg_id(),
            None,
            None,
            vec![to, subject, body],
        );
        self.send(msg.serialize()).await
    }

    async fn tweet(&mut self, msg: &str) -> Result<()> {
        let msg = Message::new(MessageType::Tweet, self.msg_id(), None, None, vec![msg]);
        self.send(msg.serialize()).await
    }

    async fn notify(&mut self, msg: &str) -> Result<()> {
        let msg = Message::new(MessageType::Notify, self.msg_id(), None, None, vec![msg]);
        self.send(msg.serialize()).await
    }

    async fn set_property(&mut self, pin: u8, prop: &str, val: &str) -> Result<()> {
        let msg = Message::new(
            MessageType::Property,
            self.msg_id(),
            None,
            None,
            vec![&pin.to_string(), prop, val],
        );
        self.send(msg.serialize()).await
    }

    async fn internal(&mut self, data: Vec<&str>) -> Result<()> {
        let msg = Message::new(MessageType::Internal, self.msg_id(), None, None, data);
        self.send(msg.serialize()).await
    }

    async fn send(&mut self, msg: Vec<u8>) -> Result<()> {
        let mut retries = conf::RETRIES_TX_MAX_NUM;
        let stream = self.stream()?;
        while retries > 0 {
            if let Err(err) = stream.write(&msg).await {
                error!("Problem sending!: {}", err);
                retries -= 1;
                thread::sleep(conf::RETRIES_TX_DELAY);
                continue;
            }
            if let Err(err) = stream.flush().await {
                error!("Problem sending!: {}", err);
                retries -= 1;
                thread::sleep(conf::RETRIES_TX_DELAY);
                continue;
            }
            info!("Sent message, awaiting reply...!!");
            return Ok(());
        }
        Err(BlynkError::MessageSend.into())
    }
}

impl Protocol for Client {
    type T = Async<TcpStream>;

    fn set_reader(&mut self, reader: BufReader<Async<TcpStream>>) {
        self.reader = Some(reader);
    }

    fn reader(&mut self) -> Option<&mut BufReader<Async<TcpStream>>> {
        self.reader.as_mut()
    }

    fn msg_id(&mut self) -> u16 {
        self.msg_id += 1;
        self.msg_id
    }

    fn disconnect(&mut self) {
        if let Ok(stream) = self.stream() {
            drop(stream);
        }
        self.msg_id = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smol::io::{Cursor, SeekFrom};

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

    #[smol_potat::test]
    async fn msg_id_incremeneted_on_send() {
        let mut client = Client {
            msg_id: 3,
            reader: None,
        };
        client.ping().await.unwrap_or_default();
        assert_eq!(4, client.msg_id)
    }
    #[smol_potat::test]
    async fn msg_id_customized() {
        let mut client = Client {
            msg_id: 3,
            reader: None,
        };
        client.response(200, 42).await.unwrap_or_default();
        // inspect the message
        assert_eq!(3, client.msg_id)
    }
    #[smol_potat::test]
    async fn propagate_send_err() {
        let mut client = Client {
            msg_id: 3,
            reader: None,
        };
        assert!(client.ping().await.is_err());
    }
    #[smol_potat::test]
    async fn ping_generates_seralized_message() {
        let reader = BufReader::with_capacity(10, Cursor::new(vec![0; 10]));
        let mut client = FakeClient {
            msg_id: 0,
            reader: Some(reader),
        };

        // intercept message into fake client
        client.ping().await.unwrap();

        let mut reader = client.reader.unwrap();
        reader.seek(SeekFrom::Start(0)).await.unwrap(); // rewind the buffer
        let buf = reader.fill_buf().await.unwrap();

        let msg = Message::new(MessageType::Ping, 1, None, None, vec![""]);
        let data = msg.serialize();
        // compare generated headers
        assert_eq!(&data[..5], &buf[..5]);
    }
    #[smol_potat::test]
    async fn read_empty_buffer_errors() {
        // try to read when the buffer is empty
        let reader = BufReader::with_capacity(0, Cursor::new(vec![0]));
        let mut client = FakeClient {
            msg_id: 0,
            reader: Some(reader),
        };
        let err = client.read().await.err().unwrap();
        assert_eq!("No message to process", err.to_string());
    }
    #[smol_potat::test]
    async fn read_message() {
        // succesful message read

        // put fake message into the buff
        let msg = Message::new(MessageType::Hw, 1, None, None, vec![""]);
        let reader = BufReader::with_capacity(10, Cursor::new(msg.serialize()));

        // intercept message into fake client
        let mut client = FakeClient {
            msg_id: 0,
            reader: Some(reader),
        };
        assert!(client.read().await.is_ok());
    }
}
