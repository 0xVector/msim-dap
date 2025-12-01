pub type Address = u32;

pub enum MsimCommand {
    SetBreakpoint(Address),
}

pub enum MsimResponse {
    Ok,
    Stopped(Address)
}