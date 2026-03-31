use super::super::message::*;
use std::io::Cursor;

/// Test request message serialization
#[test]
pub fn request_serialize() {
    let gold_serialized: &[u8] = &[
        0x01, // Request::SetBreakpoint
        0x11, 0x22, 0x33, 0x44,
    ];
    let message = Request::SetBreakpoint(0x11223344);
    assert_serialized_message_eq(gold_serialized, &*serialize_request(&message));
}

/// Test response message deserialization
#[test]
pub fn response_deserialize() -> Result<()> {
    let gold_message = Inbound::Response(ResponseKind::UnspecifiedError);
    let serialized: &[u8] = &[
        0x00, // Inbound::Response
        ResponseKind::UnspecifiedError as u8,
    ];

    let message = deserialize_inbound(&serialized)?;
    assert_inbound_message_eq(&gold_message, &message);
    Ok(())
}

/// Test event message deserialization
#[test]
pub fn event_deserialize() -> Result<()> {
    let gold_message = Inbound::Event(EventKind::StoppedAt(0x12345678));
    let serialized: &[u8] = &[
        0x01, // Inbound::Event
        0x01, // EventKind::StoppedAt
        0x12, 0x34, 0x56, 0x78,
    ];

    let message = deserialize_inbound(&serialized)?;
    assert_inbound_message_eq(&gold_message, &message);
    Ok(())
}

pub fn serialize_request(message: &Request) -> Vec<u8> {
    let mut serialized = Vec::new();
    message
        .write(&mut serialized)
        .expect("serialization failed");
    serialized
}

pub fn deserialize_inbound(serialized: &[u8]) -> Result<Inbound> {
    let mut reader = Cursor::new(serialized);
    Inbound::read(&mut reader)
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

pub fn assert_inbound_message_eq(gold_message: &Inbound, message: &Inbound) {
    assert_eq!(
        gold_message, message,
        "response message type deserialization failed"
    );
}
