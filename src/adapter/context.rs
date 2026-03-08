use super::server::DapServer;
use super::state::State;
use crate::dwarf::DwarfIndex;
use crate::msim::MsimConnection;

pub struct Context<'a> {
    pub state: &'a mut State,
    pub server: &'a mut DapServer,
    pub connection: &'a mut dyn MsimConnection,
    pub index: &'a DwarfIndex,
}

impl<'a> Context<'a> {
    pub fn new(
        state: &'a mut State,
        server: &'a mut DapServer,
        commander: &'a mut impl MsimConnection,
        index: &'a DwarfIndex,
    ) -> Self {
        Context {
            state,
            server,
            connection: commander,
            index,
        }
    }
}
