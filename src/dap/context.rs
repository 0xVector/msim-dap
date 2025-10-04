use crate::dap::server::DapServer;
use crate::dap::state::State;
use crate::dwarf::DwarfIndex;

pub struct Context<'a> {
    pub state: &'a mut State,
    pub server: &'a mut DapServer,
    pub index: &'a DwarfIndex,
}

impl<'a> Context<'a> {
    pub fn new(state: &'a mut State, server: &'a mut DapServer, index: &'a DwarfIndex) -> Self {
        Context {
            state,
            server,
            index,
        }
    }
}