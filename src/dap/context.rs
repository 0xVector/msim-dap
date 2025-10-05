use super::server::DapServer;
use super::state::State;
use crate::dwarf::DwarfIndex;
use crate::msim::Commands;

pub struct Context<'a> {
    pub state: &'a mut State,
    pub server: &'a mut DapServer,
    pub commander: &'a mut dyn Commands,
    pub index: &'a DwarfIndex,
}

impl<'a> Context<'a> {
    pub fn new(
        state: &'a mut State,
        server: &'a mut DapServer,
        commander: &'a mut impl Commands,
        index: &'a DwarfIndex,
    ) -> Self {
        Context {
            state,
            server,
            commander,
            index,
        }
    }
}
