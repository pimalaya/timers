use io_stream::Io;

use crate::{Request, Response};

use super::SendRequest;

#[derive(Debug)]
pub struct GetTimer {
    send: SendRequest,
}

impl GetTimer {
    pub fn new() -> Self {
        let send = SendRequest::new(Request::Get);
        Self { send }
    }

    pub fn resume(&mut self, input: Option<Io>) -> Result<Response, Io> {
        self.send.resume(input)
    }
}
