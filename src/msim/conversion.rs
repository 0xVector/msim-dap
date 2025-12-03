use crate::msim::commands::{MsimRequest, MsimResponse};
use crate::msim::message::{RequestMessage, RequestType, ResponseMessage, ResponseType};

impl From<MsimRequest> for RequestMessage {
    fn from(command: MsimRequest) -> Self {
        let msg_type = match command {
            MsimRequest::SetBreakpoint(_) => RequestType::SetBreakpoint,
            MsimRequest::Continue => RequestType::Continue,
        };

        let address = match command {
            MsimRequest::SetBreakpoint(address) => address,
            MsimRequest::Continue => Default::default(),
        };

        RequestMessage { msg_type, address }
    }
}

impl From<ResponseMessage> for MsimResponse {
    fn from(message: ResponseMessage) -> Self {
        match message.msg_type {
            ResponseType::Ok => MsimResponse::Ok,
            ResponseType::StoppedAt => MsimResponse::Stopped(message.address),
        }
    }
}
