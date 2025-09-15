use crate::protocol::protocol::DapServer;
use dap::requests::{AttachRequestArguments, InitializeArguments};
use dap::responses::ResponseBody;
use dap::types::Capabilities;

pub trait Handles {
    fn initialize(server: &mut DapServer, args: &InitializeArguments) -> ResponseBody;
    fn attach(server: &mut DapServer, args: &AttachRequestArguments) -> ResponseBody;
}

pub struct Handler {}

impl Handles for Handler {
    fn initialize(_server: &mut DapServer, args: &InitializeArguments) -> ResponseBody {
        if let Some(name) = &args.client_name {
            println!("New client: {}, {}", name, args.adapter_id);
        }

        ResponseBody::Initialize(Capabilities {
            ..Default::default() // No extra capabilities advertised
        })
    }

    fn attach(server: &mut DapServer, args: &AttachRequestArguments) -> ResponseBody {
        println!("Attach request");
        server.send_event(dap::events::Event::Initialized).expect("Server error");
        ResponseBody::Attach
    }
}
