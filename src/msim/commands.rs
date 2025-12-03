pub type Address = u32;

pub enum MsimRequest {
    SetBreakpoint(Address),
    Continue,
}

pub enum MsimResponse {
    Ok,
    Stopped(Address),
}
