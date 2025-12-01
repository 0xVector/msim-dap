use super::super::Result;
use super::super::message::*;
use std::io::Cursor;

/// Test message serialization
#[test]
pub fn message_serialize() {
    let gold_serialized: &[u8] = &[0x01, 0x11, 0x22, 0x33, 0x44];

    let message = Message {
        msg_type: MessageType::Breakpoint,
        address: 0x11223344,
    };

    assert_serialized_message_eq(gold_serialized, &*serialize(&message));
}

/// Test message deserialization
#[test]
pub fn message_deserialize() -> Result<()> {
    let gold_message = Message {
        msg_type: MessageType::NoOp,
        address: 0x12345678,
    };

    let serialized: &[u8] = &[0x00, 0x12, 0x34, 0x56, 0x78];

    let message = deserialize(&serialized)?;
    assert_message_eq(&gold_message, &message);
    Ok(())
}

/// Test message serialization and deserialization roundtrip
#[test]
pub fn message_roundtrip() -> Result<()> {
    let message = Message {
        msg_type: MessageType::StoppedAt,
        address: 0x0abcdef0,
    };

    let serialized = serialize(&message);
    let deserialized = deserialize(&serialized)?;

    assert_message_eq(&message, &deserialized);
    Ok(())
}

pub fn serialize(message: &Message) -> Vec<u8> {
    let mut serialized = Vec::new();
    message
        .write(&mut serialized)
        .expect("serialization failed");
    serialized
}

pub fn deserialize(serialized: &[u8]) -> Result<Message> {
    let mut reader = Cursor::new(serialized);
    Message::read(&mut reader)
}

pub fn assert_serialized_message_eq(gold_serialized: &[u8], serialized: &[u8]) {
    assert_eq!(
        gold_serialized[0], serialized[0],
        "message type serialization failed"
    );
    assert_eq!(
        gold_serialized[1..5],
        serialized[1..5],
        "address serialization failed"
    );
    assert_eq!(gold_serialized, serialized, "message serialization failed");
}

pub fn assert_message_eq(gold_message: &Message, message: &Message) {
    assert_eq!(
        gold_message.msg_type, message.msg_type,
        "message type deserialization failed"
    );
    assert_eq!(
        gold_message.address, message.address,
        "address deserialization failed"
    );
}
