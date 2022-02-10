use std::error::Error;
use std::io::prelude::*;
use std::io::BufReader;
use std::net::{Shutdown, TcpStream};
use std::thread;
use std::time::Duration;

use log::*;

use crate::conf;
use crate::message::{Message, MessageType, ProtocolHeader};
const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Default)]
pub struct Client {
    msg_id: u16,
    reader: Option<BufReader<TcpStream>>,
}

impl Client {
    pub fn set_read_timeout(&self, duration: Duration) {
        if let Ok(stream) = self.stream() {
            stream
                .set_read_timeout(Some(duration))
                .expect("read timeout problem");
        }
    }

    pub fn read(&mut self) -> Result<Message, Box<dyn Error>> {
        let reader: &mut BufReader<TcpStream> =
            self.reader.as_mut().ok_or("Unable to access reader")?;
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            return Err("No message to process".into());
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

    fn msg_id(&mut self) -> u16 {
        self.msg_id += 1;
        self.msg_id
    }

    pub fn set_stream(&mut self, stream: TcpStream) {
        self.reader = Some(BufReader::new(stream));
    }

    fn stream(&self) -> Result<&TcpStream, Box<dyn Error>> {
        if let Some(r) = self.reader.as_ref() {
            return Ok(r.get_ref());
        }
        Err("Stream not available".into())
    }

    pub fn disconnect(&mut self) {
        if let Ok(stream) = self.stream() {
            stream
                .shutdown(Shutdown::Both)
                .unwrap_or_else(|err| error!("shutdown call failed, with err {}", err));
        }
        self.msg_id = 0;
    }

    pub fn login(&mut self, token: &str) -> Result<(), Box<dyn Error>> {
        let msg = Message::new(MessageType::Login, self.msg_id(), None, None, vec![token]);
        send(self.stream()?, msg.serialize())
    }

    pub fn heartbeat(
        &mut self,
        heartbeat: Duration,
        rcv_buffer: u16,
    ) -> Result<(), Box<dyn Error>> {
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

        send(self.stream()?, msg.serialize())
    }

    pub fn ping(&mut self) -> Result<(), Box<dyn Error>> {
        let msg = Message::new(MessageType::Ping, self.msg_id(), None, None, vec![]);
        send(self.stream()?, msg.serialize())
    }

    pub fn response(&self, status: u16, msg_id: u16) -> Result<(), Box<dyn Error>> {
        let msg = Message::new(
            MessageType::Rsp,
            msg_id,
            None,
            None,
            vec![&status.to_string()],
        );
        send(self.stream()?, msg.serialize())
    }

    pub fn virtual_write(&mut self, v_pin: u8, val: &str) -> Result<(), Box<dyn Error>> {
        let msg = Message::new(
            MessageType::Hw,
            self.msg_id(),
            None,
            None,
            vec!["vw", &v_pin.to_string(), val],
        );
        send(self.stream()?, msg.serialize())
    }

    pub fn virtual_sync(&mut self, pins: Vec<u32>) -> Result<(), Box<dyn Error>> {
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
        send(self.stream()?, msg.serialize())
    }

    pub fn email(&mut self, to: &str, subject: &str, body: &str) -> Result<(), Box<dyn Error>> {
        let msg = Message::new(
            MessageType::Email,
            self.msg_id(),
            None,
            None,
            vec![to, subject, body],
        );
        send(self.stream()?, msg.serialize())
    }

    pub fn tweet(&mut self, msg: &str) -> Result<(), Box<dyn Error>> {
        let msg = Message::new(MessageType::Tweet, self.msg_id(), None, None, vec![msg]);
        send(self.stream()?, msg.serialize())
    }

    pub fn notify(&mut self, msg: &str) -> Result<(), Box<dyn Error>> {
        let msg = Message::new(MessageType::Notify, self.msg_id(), None, None, vec![msg]);
        send(self.stream()?, msg.serialize())
    }

    pub fn set_property(&mut self, pin: u8, prop: &str, val: &str) -> Result<(), Box<dyn Error>> {
        let msg = Message::new(
            MessageType::Property,
            self.msg_id(),
            None,
            None,
            vec![&pin.to_string(), prop, val],
        );
        send(self.stream()?, msg.serialize())
    }

    pub fn internal(&mut self, data: Vec<&str>) -> Result<(), Box<dyn Error>> {
        let msg = Message::new(MessageType::Internal, self.msg_id(), None, None, data);
        send(self.stream()?, msg.serialize())
    }
}

fn send(mut stream: &TcpStream, msg: Vec<u8>) -> Result<(), Box<dyn Error>> {
    let mut retries = conf::RETRIES_TX_MAX_NUM;
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
        println!("Sent message, awaiting reply...!!");
        return Ok(());
    }
    Err("Unable to send the message".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msg_id_incremeneted_on_send() {}
    #[test]
    fn msg_id_customized() {}
    #[test]
    fn propagate_send_err() {}

    #[test]
    fn disconnect_with_no_stream() {}

    #[test]
    fn ping_generates_seralized_message() {}
    #[test]
    fn read_empty_buffer() {}
    #[test]
    fn read_message() {}
}
