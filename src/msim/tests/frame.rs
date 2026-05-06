#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::super::frame::*;
use std::io::Cursor;

// --- Helpers ---

pub fn serialize_request(frame: &Request) -> Vec<u8> {
    let mut serialized = Vec::new();
    frame.write(&mut serialized).expect("serialization failed");
    serialized
}

pub fn deserialize_inbound(serialized: &[u8]) -> Result<Inbound> {
    let mut reader = Cursor::new(serialized);
    Inbound::read(&mut reader)
}

/// Assert a serialized request frame matches a gold byte sequence.
/// Full 25-byte comparison: [kind(1)] [arg0(8 BE)] [arg1(8 BE)] [arg2(8 BE)]
pub fn assert_serialized_eq(gold: &[u8; OUTBOUND_FRAME_SIZE], actual: &[u8]) {
    assert_eq!(actual.len(), OUTBOUND_FRAME_SIZE, "wrong frame length");
    assert_eq!(gold[0], actual[0], "kind byte mismatch");
    assert_eq!(&gold[1..9], &actual[1..9], "arg0 mismatch");
    assert_eq!(&gold[9..17], &actual[9..17], "arg1 mismatch");
    assert_eq!(&gold[17..25], &actual[17..25], "arg2 mismatch");
}

pub fn assert_inbound_eq(gold: &Inbound, frame: &Inbound) {
    assert_eq!(gold, frame, "inbound frame mismatch");
}

// --- Request serialization tests ---

#[test]
fn request_serialize_no_args() {
    let mut gold = [0u8; OUTBOUND_FRAME_SIZE];
    gold[0] = 0x01;
    assert_serialized_eq(&gold, &serialize_request(&Request::Resume));

    let mut gold = [0u8; OUTBOUND_FRAME_SIZE];
    gold[0] = 0x02;
    assert_serialized_eq(&gold, &serialize_request(&Request::Pause));

    // ReadPC(cpu=0): all args zero
    let mut gold = [0u8; OUTBOUND_FRAME_SIZE];
    gold[0] = 0x0D;
    assert_serialized_eq(&gold, &serialize_request(&Request::ReadPC(0)));
}

#[test]
fn request_serialize_single_arg() {
    // SetCodeBreakpoint(0x11223344): kind=0x05, arg0=0x0000000011223344, arg1=0, arg2=0
    let frame = Request::SetCodeBreakpoint(0x1122_3344);
    let gold: [u8; OUTBOUND_FRAME_SIZE] = [
        0x05, // kind
        0x00, 0x00, 0x00, 0x00, 0x11, 0x22, 0x33, 0x44, // arg0 BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg1 BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg2 BE
    ];
    assert_serialized_eq(&gold, &serialize_request(&frame));
}

#[test]
fn request_serialize_step() {
    // Step(42): kind=0x04, arg0=42, arg1=0, arg2=0
    let frame = Request::Step(2, 42);
    let gold: [u8; OUTBOUND_FRAME_SIZE] = [
        0x04, // kind
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // arg0 BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2A, // arg1 = 42 BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg2 BE
    ];
    assert_serialized_eq(&gold, &serialize_request(&frame));
}

#[test]
fn request_serialize_two_args() {
    // WritePC { cpu: 0, value: 0xDEADBEEF }: kind=0x0E, arg0=cpu, arg1=value, arg2=0
    let frame = Request::WritePC {
        cpu: 0,
        value: 0xDEAD_BEEF,
    };
    let gold: [u8; OUTBOUND_FRAME_SIZE] = [
        0x0E, // kind
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg0 = cpu=0 BE
        0x00, 0x00, 0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF, // arg1 = 0xDEADBEEF BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg2 BE
    ];
    assert_serialized_eq(&gold, &serialize_request(&frame));
}

#[test]
fn request_serialize_three_args() {
    // WriteGeneralRegister { cpu: 2, reg: 5, value: 0xDEADBEEF }: kind=0x0A, arg0=cpu, arg1=reg, arg2=value
    let frame = Request::WriteGeneralRegister {
        cpu: 2,
        reg: 5,
        value: 0xDEAD_BEEF,
    };
    let gold: [u8; OUTBOUND_FRAME_SIZE] = [
        0x0A, // kind
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // arg0 = cpu=2 BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, // arg1 = reg=5 BE
        0x00, 0x00, 0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF, // arg2 = 0xDEADBEEF BE
    ];
    assert_serialized_eq(&gold, &serialize_request(&frame));
}

#[test]
fn request_serialize_data_breakpoint() {
    // SetPhysDataBreakpoint { address: 0x1000, kind: ReadWrite }: kind=0x07, arg0=0x1000, arg1=3, arg2=0
    let frame = Request::SetPhysDataBreakpoint {
        address: 0x1000,
        kind: DataBreakpointKind::ReadWrite,
    };
    let gold: [u8; OUTBOUND_FRAME_SIZE] = [
        0x07, // kind
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, // arg0 = 0x1000 BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, // arg1 = 3 (ReadWrite) BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg2 BE
    ];
    assert_serialized_eq(&gold, &serialize_request(&frame));
}

#[test]
fn request_serialize_write_memory() {
    // WritePhysMemory { address: 0x2000, data: 0x0102030405060708 }: arg2=0
    let frame = Request::WritePhysMemory {
        address: 0x2000,
        data: 0x0102_0304_0506_0708,
    };
    let gold: [u8; OUTBOUND_FRAME_SIZE] = [
        0x10, // kind
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, // arg0 = 0x2000 BE
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // arg1 = data BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg2 BE
    ];
    assert_serialized_eq(&gold, &serialize_request(&frame));
}

#[test]
fn request_serialize_virt_memory_with_cpu() {
    // ReadVirtMemory { cpu: 1, address: 0xABCD }: arg0=cpu, arg1=address, arg2=0
    let frame = Request::ReadVirtMemory {
        cpu: 1,
        address: 0xABCD,
    };
    let gold: [u8; OUTBOUND_FRAME_SIZE] = [
        0x11, // kind
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // arg0 = cpu=1 BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xAB, 0xCD, // arg1 = 0xABCD BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg2 BE
    ];
    assert_serialized_eq(&gold, &serialize_request(&frame));
}

// --- Inbound deserialization tests ---

#[test]
fn response_deserialize_ok() -> Result<()> {
    let gold = Inbound::Response {
        status: ResponseStatus::Ok,
        arg0: 0x100,
        arg1: 0,
        arg2: 0,
    };
    let serialized: [u8; INBOUND_FRAME_SIZE] = [
        0x01, // category: Response
        0x01, // status: Ok
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, // arg0 = 0x100 BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg1 BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg2 BE
    ];
    let frame = deserialize_inbound(&serialized)?;
    assert_inbound_eq(&gold, &frame);
    Ok(())
}

#[test]
fn response_deserialize_error() -> Result<()> {
    let gold = Inbound::Response {
        status: ResponseStatus::UnspecifiedError,
        arg0: 0,
        arg1: 0,
        arg2: 0,
    };
    let serialized: [u8; INBOUND_FRAME_SIZE] = [
        0x01, // category: Response
        0x02, // status: Error
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg0
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg1
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg2
    ];
    let frame = deserialize_inbound(&serialized)?;
    assert_inbound_eq(&gold, &frame);
    Ok(())
}

#[test]
fn event_deserialize_stopped_at() -> Result<()> {
    let gold = Inbound::Event {
        kind: EventKind::StoppedAt,
        arg0: 1,           // CPU ID
        arg1: 0x1234_5678, // address
        arg2: 2,
    };
    let serialized: [u8; INBOUND_FRAME_SIZE] = [
        0x02, // category: Event
        0x02, // kind: StoppedAt
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // arg0 = CPU ID BE
        0x00, 0x00, 0x00, 0x00, 0x12, 0x34, 0x56, 0x78, // arg1 = address BE
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // arg2 = reason BE
    ];
    let frame = deserialize_inbound(&serialized)?;
    assert_inbound_eq(&gold, &frame);
    Ok(())
}

#[test]
fn event_deserialize_exited() -> Result<()> {
    let gold = Inbound::Event {
        kind: EventKind::Terminated,
        arg0: 0,
        arg1: 0,
        arg2: 0,
    };
    let serialized: [u8; INBOUND_FRAME_SIZE] = [
        0x02, // category: Event
        0x01, // kind: Exited
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg0
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg1
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // arg2
    ];
    let frame = deserialize_inbound(&serialized)?;
    assert_inbound_eq(&gold, &frame);
    Ok(())
}

// --- Error / rejection tests ---

#[test]
fn inbound_rejects_unknown_category() {
    let serialized: [u8; INBOUND_FRAME_SIZE] = [
        0xFF, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert!(
        deserialize_inbound(&serialized).is_err(),
        "unknown category should fail"
    );
}

#[test]
fn inbound_rejects_uninitialized_category() {
    let serialized: [u8; INBOUND_FRAME_SIZE] = [
        0x00, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert!(
        deserialize_inbound(&serialized).is_err(),
        "uninitialized category 0x00 should fail"
    );
}

#[test]
fn inbound_rejects_unknown_event_kind() {
    let serialized: [u8; INBOUND_FRAME_SIZE] = [
        0x02, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert!(
        deserialize_inbound(&serialized).is_err(),
        "unknown event kind should fail"
    );
}

#[test]
fn inbound_rejects_uninitialized_response_status() {
    let serialized: [u8; INBOUND_FRAME_SIZE] = [
        0x01, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert!(
        deserialize_inbound(&serialized).is_err(),
        "uninitialized response status 0x00 should fail"
    );
}

#[test]
fn inbound_truncated_returns_error() {
    let short: &[u8] = &[0x01, 0x01, 0x00];
    assert!(
        deserialize_inbound(short).is_err(),
        "truncated frame should fail"
    );
}
