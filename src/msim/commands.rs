pub type Address = u32;

pub enum MsimRequest {
    SetBreakpoint(Address),
}

pub enum MsimResponse {
    Ok,
    Stopped(Address),
}
