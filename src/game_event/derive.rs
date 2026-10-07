use bytes::Bytes;

use crate::event::EventManager;

pub use macro_derive::GameEvent;

pub type ListKeysT = [(u32, String)];

/// `CMsgSource1LegacyGameEvent`, hand-written so `val_string` decodes as bytes:
/// demos carry strings cut mid-codepoint, which prost's `string` rejects.
#[derive(Clone, PartialEq, prost::Message)]
pub struct LegacyGameEvent {
    #[prost(int32, optional, tag = "2")]
    pub eventid: Option<i32>,
    #[prost(message, repeated, tag = "3")]
    pub keys: Vec<KeyT>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct KeyT {
    #[prost(bytes = "bytes", optional, tag = "2")]
    pub val_string: Option<Bytes>,
    #[prost(float, optional, tag = "3")]
    pub val_float: Option<f32>,
    #[prost(int32, optional, tag = "4")]
    pub val_long: Option<i32>,
    #[prost(int32, optional, tag = "5")]
    pub val_short: Option<i32>,
    #[prost(int32, optional, tag = "6")]
    pub val_byte: Option<i32>,
    #[prost(bool, optional, tag = "7")]
    pub val_bool: Option<bool>,
    #[prost(uint64, optional, tag = "8")]
    pub val_uint64: Option<u64>,
}

pub type GameEventSerializerFactory =
    fn(keys: &ListKeysT) -> Result<Box<dyn GameEventSerializer>, std::io::Error>;

pub trait GameEventSerializer: Send + Sync {
    fn parse_and_dispatch_event(
        &self,
        keys: Vec<KeyT>,
        event_manager: &mut EventManager,
        state: &crate::CsDemoParserState,
    ) -> Result<(), std::io::Error>;
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::LegacyGameEvent;
    use crate::protobuf::CMsgSource1LegacyGameEvent;

    #[test]
    fn non_utf8_val_string_decodes() {
        // keys[0].val_string = [0xa0, 0x5f]
        let wire = [0x1a, 0x04, 0x12, 0x02, 0xa0, 0x5f];

        assert!(CMsgSource1LegacyGameEvent::decode(&wire[..]).is_err());

        let msg = LegacyGameEvent::decode(&wire[..]).unwrap();
        assert_eq!(msg.keys[0].val_string.as_deref(), Some(&[0xa0, 0x5f][..]));
    }
}
