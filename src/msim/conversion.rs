use crate::msim::commands::MsimRequest::SetBreakpoint;
use crate::msim::commands::{MsimRequest, MsimResponse};
use crate::msim::message::{RequestMessage, RequestType, ResponseMessage, ResponseType};

impl From<MsimRequest> for RequestMessage {
    fn from(command: MsimRequest) -> Self {
        match command {
            SetBreakpoint(address) => RequestMessage {
                msg_type: RequestType::SetBreakpoint,
                address,
            },
        }
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
