#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unnecessary_wraps,
    unused
)]

use crate::dwarf::DebugIndex;
use crate::msim::{ArgType, Connection, RawResponse, Request, ResponseStatus, Result};
use crate::target::{DebugTarget, MsimTarget};
use crate::{Address, LineNo};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

type HandlerT = Box<dyn FnMut(&Request) -> Result<RawResponse>>;
type ConnectionT = Rc<RefCell<MockConnection>>;
type TargetT = MsimTarget<ConnectionT, MockIndex>;

/// Mock test connection that records sent requests and allows for custom response handling.
struct MockConnection {
    handler: HandlerT,
    pub sent: Vec<Request>,
}

impl MockConnection {
    fn new(handler: HandlerT) -> Self {
        Self {
            handler,
            sent: Vec::new(),
        }
    }
}

impl Connection for MockConnection {
    fn send(&mut self, request: Request) -> Result<RawResponse> {
        self.sent.push(request);
        (self.handler)(&request)
    }
}

impl Connection for Rc<RefCell<MockConnection>> {
    fn send(&mut self, request: Request) -> Result<RawResponse> {
        self.borrow_mut().send(request)
    }
}

fn assert_was_sent(conn: &ConnectionT, expected: Request) {
    let sent = &conn.borrow().sent;
    assert!(
        sent.contains(&expected),
        "Expected request {expected:?} was not sent. Sent requests: {sent:?}"
    );
}

fn assert_were_sent(conn: &ConnectionT, expected: &[Request]) {
    let sent = &conn.borrow().sent;
    for req in expected {
        assert!(
            sent.contains(req),
            "Expected request {req:?} was not sent. Sent requests: {sent:?}"
        );
    }
}

fn assert_sent_exactly(conn: &ConnectionT, expected: &[Request]) {
    let sent = &conn.borrow().sent;
    sent.iter().zip(expected).for_each(|(s, e)| {
        assert_eq!(
            s, e,
            "Expected request {e:?} was not sent. Sent requests: {sent:?}"
        );
    });
    assert_eq!(
        sent.len(),
        expected.len(),
        "Expected exactly {} requests to be sent, but {} were sent. Sent requests: {sent:?}",
        expected.len(),
        sent.len()
    );
}

#[derive(Default)]
struct MockIndex {
    pub map: HashMap<(PathBuf, LineNo), Address>,
}

impl MockIndex {
    fn with(entries: &[(&str, LineNo, Address)]) -> Self {
        let map = entries
            .iter()
            .map(|(p, l, a)| ((PathBuf::from(p), *l), *a))
            .collect();
        Self { map }
    }
}

impl DebugIndex for MockIndex {
    fn get_address(
        &self,
        file_path: &std::path::Path,
        line: crate::LineNo,
    ) -> Option<crate::Address> {
        self.map.get(&(file_path.to_path_buf(), line)).copied()
    }

    fn resolve_address(
        &self,
        address: crate::Address,
    ) -> Option<(&std::path::Path, crate::LineNo)> {
        self.map.iter().find_map(|((path, line), &addr)| {
            if addr == address {
                Some((path.as_path(), *line))
            } else {
                None
            }
        })
    }
}

fn make_target_index(
    handler: impl FnMut(&Request) -> Result<RawResponse> + 'static,
    index: MockIndex,
) -> (TargetT, ConnectionT) {
    let conn = Rc::new(RefCell::new(MockConnection::new(Box::new(handler))));
    (MsimTarget::new(Rc::clone(&conn), index), conn)
}

fn make_target(
    handler: impl FnMut(&Request) -> Result<RawResponse> + 'static,
) -> (TargetT, ConnectionT) {
    make_target_index(handler, MockIndex::default())
}

fn k() -> Result<RawResponse> {
    Ok(RawResponse {
        status: ResponseStatus::Ok,
        arg0: 0,
        arg1: 0,
        arg2: 0,
    })
}

fn ok(arg0: ArgType) -> Result<RawResponse> {
    Ok(RawResponse {
        status: ResponseStatus::Ok,
        arg0,
        arg1: 0,
        arg2: 0,
    })
}

fn ok_resp(arg0: ArgType, arg1: ArgType, arg2: ArgType) -> Result<RawResponse> {
    Ok(RawResponse {
        status: ResponseStatus::Ok,
        arg0,
        arg1,
        arg2,
    })
}

fn err() -> Result<RawResponse> {
    Ok(RawResponse {
        status: ResponseStatus::UnspecifiedError,
        arg0: 0,
        arg1: 0,
        arg2: 0,
    })
}

#[test]
fn cpu_count_returns_correct_value() {
    let (mut target, conn) = make_target(|req| match req {
        Request::GetConfig => ok(4),
        Request::GetCpuInfo(_) => ok(01),
        _ => panic!("Unexpected request: {req:?}"),
    });
    assert_eq!(target.cpu_count().unwrap(), 4);
    assert_was_sent(&conn, Request::GetConfig);
}

#[test]
fn resume_success() {
    let (mut target, conn) = make_target(|req| match req {
        Request::Resume => k(),
        _ => panic!("Unexpected request: {req:?}"),
    });
    assert!(target.resume().is_ok());
    assert_was_sent(&conn, Request::Resume);
}

#[test]
fn pause_success() {
    let (mut target, conn) = make_target(|req| match req {
        Request::Pause => k(),
        _ => panic!("Unexpected request: {req:?}"),
    });
    assert!(target.pause().is_ok());
    assert_was_sent(&conn, Request::Pause);
}

#[test]
fn terminate_success() {
    let (mut target, conn) = make_target(|req| match req {
        Request::Terminate => k(),
        _ => panic!("Unexpected request: {req:?}"),
    });
    assert!(target.terminate().is_ok());
    assert_was_sent(&conn, Request::Terminate);
}

#[test]
fn step_by_success() {
    let (mut target, conn) = make_target(|req| match req {
        Request::Step(cpu, count) => {
            assert_eq!(*cpu, 1);
            assert_eq!(*count, 5);
            k()
        }
        Request::Resume => k(),
        _ => panic!("Unexpected request: {req:?}"),
    });
    assert!(target.step_by(1, 5).is_ok());
    assert_was_sent(&conn, Request::Step(1, 5));
}

#[test]
fn set_code_bp_success() {
    let (mut target, conn) = make_target_index(
        |req| match req {
            Request::SetCodeBreakpoint(addr) => {
                assert_eq!(*addr, 1234);
                k()
            }
            _ => panic!("Unexpected request: {req:?}"),
        },
        MockIndex::with(&[("test.c", 10, 1234)]),
    );
    let addr = target
        .set_code_bp(PathBuf::from("test.c").as_path(), 10)
        .unwrap();
    assert_eq!(addr, 1234);
    assert_was_sent(&conn, Request::SetCodeBreakpoint(1234));
}

#[test]
fn set_code_bp_address_not_found() {
    let (mut target, _) = make_target(|_req| panic!("Unexpected request"));
    let err = target
        .set_code_bp(PathBuf::from("test.c").as_path(), 10)
        .unwrap_err();
    assert!(matches!(
        err,
        crate::target::TargetError::AddressNotFound(_, _)
    ));
}

//noinspection ALL
#[test]
fn set_code_bp_does_not_set_duplicate() {
    let (mut target, conn) = make_target_index(
        |req| match req {
            Request::SetCodeBreakpoint(addr) => {
                assert_eq!(*addr, 1234);
                k()
            }
            _ => panic!("Unexpected request: {req:?}"),
        },
        MockIndex::with(&[("test.c", 10, 1234)]),
    );
    // First call should succeed
    let addr1 = target
        .set_code_bp(PathBuf::from("test.c").as_path(), 10)
        .unwrap();
    assert_eq!(addr1, 1234);
    // Second call should also succeed but not send a new request
    let addr2 = target
        .set_code_bp(PathBuf::from("test.c").as_path(), 10)
        .unwrap();
    assert_eq!(addr2, 1234);
    assert_was_sent(&conn, Request::SetCodeBreakpoint(1234));
    assert_eq!(
        conn.borrow().sent.len(),
        1,
        "Expected only one SetCodeBreakpoint request to be sent"
    );
}

#[test]
fn remove_code_bp_success() {
    let (mut target, conn) = make_target_index(
        |req| match req {
            Request::RemoveCodeBreakpoint(addr) => {
                assert_eq!(*addr, 1234);
                k()
            }
            Request::SetCodeBreakpoint(_) => k(),
            _ => panic!("Unexpected request: {req:?}"),
        },
        MockIndex::with(&[("test.c", 10, 1234)]),
    );
    target
        .set_code_bp(PathBuf::from("test.c").as_path(), 10)
        .unwrap();
    let addr = target
        .remove_code_bp(PathBuf::from("test.c").as_path(), 10)
        .unwrap();
    assert_eq!(addr, 1234);
    assert_was_sent(&conn, Request::RemoveCodeBreakpoint(1234));
}

#[test]
fn remove_code_bp_address_not_found() {
    let (mut target, _) = make_target(|_req| panic!("Unexpected request"));
    let err = target
        .remove_code_bp(PathBuf::from("test.c").as_path(), 10)
        .unwrap_err();
    assert!(matches!(
        err,
        crate::target::TargetError::AddressNotFound(_, _)
    ));
}

#[test]
fn remove_code_bp_idempotent() {
    let (mut target, conn) = make_target_index(
        |req| match req {
            Request::RemoveCodeBreakpoint(addr) => {
                assert_eq!(*addr, 1234);
                k()
            }
            Request::SetCodeBreakpoint(_) => k(),
            _ => panic!("Unexpected request: {req:?}"),
        },
        MockIndex::with(&[("test.c", 10, 1234)]),
    );
    target
        .set_code_bp(PathBuf::from("test.c").as_path(), 10)
        .unwrap();
    // First call should succeed
    let addr1 = target
        .remove_code_bp(PathBuf::from("test.c").as_path(), 10)
        .unwrap();
    assert_eq!(addr1, 1234);
    // Second call should also succeed but not send a new request
    let addr2 = target
        .remove_code_bp(PathBuf::from("test.c").as_path(), 10)
        .unwrap();
    assert_eq!(addr2, 1234);
    assert_sent_exactly(
        &conn,
        &[
            Request::SetCodeBreakpoint(1234),
            Request::RemoveCodeBreakpoint(1234),
        ],
    );
    assert_eq!(
        conn.borrow().sent.len(),
        2,
        "Expected only one RemoveCodeBreakpoint request to be sent"
    );
}

#[test]
fn replace_code_bps_replaces_previous() {
    let (mut target, conn) = make_target_index(
        |req| match req {
            Request::SetCodeBreakpoint(addr) | Request::RemoveCodeBreakpoint(addr) => {
                assert!(*addr == 1234 || *addr == 5678);
                k()
            }
            _ => panic!("Unexpected request: {req:?}"),
        },
        MockIndex::with(&[("test.c", 10, 1234), ("test.c", 20, 5678)]),
    );
    target
        .set_code_bp(PathBuf::from("test.c").as_path(), 10)
        .unwrap();
    let results = target.replace_code_bps(PathBuf::from("test.c").as_path(), &[20]);
    assert_eq!(results.len(), 1);
    assert!(results[0].is_ok());
    assert_sent_exactly(
        &conn,
        &[
            Request::SetCodeBreakpoint(1234),
            Request::RemoveCodeBreakpoint(1234),
            Request::SetCodeBreakpoint(5678),
        ],
    );
}
