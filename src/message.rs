use num_enum::TryFromPrimitive;
use std::convert::TryFrom;
use std::error::Error;

#[derive(TryFromPrimitive, Debug, Clone, Copy)]
#[repr(u8)]
pub enum MessageType {
    Rsp = 0,
    Login = 2,
    Ping = 6,
    Tweet = 12,
    Email = 13,
    Notify = 14,
    Bridge = 15,
    HwSync = 16,
    Internal = 17,
    Property = 19,
    Hw = 20,
    Redirect = 41,
}

#[derive(restruct_derive::Struct)]
#[fmt = "!BHH"]
pub struct ProtocolHeader;

#[derive(TryFromPrimitive, Debug)]
#[repr(u16)]
pub enum ProtocolStatus {
    StatusInvalidToken = 9,
    StatusNoData = 17,
    StatusOk = 200,
    VpinMaxNum = 32,
}

#[derive(Debug)]
pub struct Message {
    pub mtype: MessageType,
    pub id: u16,
    pub size: Option<u16>,
    pub status: Option<ProtocolStatus>,
    pub body: Vec<String>,
}

impl Message {
    pub fn new(
        mtype: MessageType,
        id: u16,
        size: Option<u16>,
        status: Option<ProtocolStatus>,
        body: Vec<&str>,
    ) -> Message {
        let body = body.iter().map(|&s| s.into()).collect();
        Message {
            mtype,
            id,
            size,
            status,
            body,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = self.body.join("\0").as_bytes().to_vec();

        let mut buffer = Vec::new();
        let input: (u8, u16, u16) = (self.mtype as u8, self.id, data.len() as u16);

        ProtocolHeader::write_to(input, &mut buffer).unwrap();
        buffer.append(&mut data);
        buffer
    }

    pub fn deserilize(mut rsp_data: &[u8]) -> Result<Message, Box<dyn Error>> {
        let mut msg_body = vec![];
        let (msg_type_raw, msg_id, h_data) = ProtocolHeader::read_from(&mut rsp_data)?;

        if msg_id == 0 {
            return Err("Invalid msg_id = 0".into());
        }

        let msg_type = MessageType::try_from(msg_type_raw)?;
        let mut size = None;
        let mut status = None;

        match msg_type {
            MessageType::Rsp | MessageType::Ping => {
                status = Some(ProtocolStatus::try_from(h_data).expect("Incorrect response status"));
            }
            MessageType::Hw
            | MessageType::Bridge
            | MessageType::Internal
            | MessageType::Redirect => {
                size = Some(h_data);
                let msg_body_raw = String::from_utf8(rsp_data[..h_data.into()].to_vec())?;
                msg_body = msg_body_raw.split('\0').map(String::from).collect();
            }
            _ => panic!("Unknown message type {:?}", msg_type),
        }

        Ok(Message::new(
            msg_type,
            msg_id,
            size,
            status,
            msg_body.iter().map(|s| s as &str).collect(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_and_deserialize() {
        let msg = Message::new(
            MessageType::Ping,
            32,
            None,
            Some(ProtocolStatus::StatusOk),
            vec![""; 201],
        );
        let data = msg.serialize();
        let dmsg = Message::deserilize(&data).unwrap();
        assert_eq!(msg.mtype as u8, dmsg.mtype as u8);
        assert_eq!(msg.id, dmsg.id);
        assert_eq!(msg.size, dmsg.size);
        assert_eq!(msg.status.unwrap() as u16, dmsg.status.unwrap() as u16);
        assert_ne!(msg.body, dmsg.body);
    }

    #[test]
    fn deserialize_response() {
        let mut data = vec!["test", "it"].join("\0").as_bytes().to_vec();

        let mut buffer = Vec::new();
        let input: (u8, u16, u16) = (MessageType::Hw as u8, 32, data.len() as u16);

        ProtocolHeader::write_to(input, &mut buffer).unwrap();
        buffer.append(&mut data);

        let dmsg = Message::deserilize(&buffer).unwrap();
        assert_eq!(MessageType::Hw as u8, dmsg.mtype as u8);
        assert_eq!(32, dmsg.id);
        assert_eq!(7, dmsg.size.unwrap());
        assert_eq!(true, dmsg.status.is_none());
        assert_eq!(vec!["test", "it"], dmsg.body);
    }

    #[test]
    fn serialize_with_payload() {
        let msg = Message::new(MessageType::Hw, 32, None, None, vec!["a", "b", "c"]);

        let data = msg.serialize();
        let header: Vec<u8> = vec![MessageType::Hw as u8, 0, 32, 0, 5];
        assert_eq!(header, &data[..5]);

        let payload: Vec<u8> = vec!['a', '\0', 'b', '\0', 'c']
            .iter()
            .map(|c| *c as u8)
            .collect::<Vec<_>>();
        assert_eq!(payload, &data[5..]);
    }
}
