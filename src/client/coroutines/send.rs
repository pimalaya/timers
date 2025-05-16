use io_stream::{
    coroutines::{Read, Write},
    Io,
};
use memchr::memrchr;

use crate::{Request, Response};

#[derive(Debug)]
pub enum State {
    SendRequest(Write),
    ReceiveResponse(Read),
}

#[derive(Debug)]
pub struct SendRequest {
    state: State,
    response: Vec<u8>,
}

impl SendRequest {
    pub fn new(request: Request) -> Self {
        let coroutine = Write::new(request.to_vec());
        let state = State::SendRequest(coroutine);
        let response = Vec::new();

        Self { state, response }
    }

    pub fn resume(&mut self, mut input: Option<Io>) -> Result<Response, Io> {
        loop {
            match &mut self.state {
                State::SendRequest(write) => {
                    write.resume(input.take())?;

                    let read = Read::default();
                    self.state = State::ReceiveResponse(read);
                }
                State::ReceiveResponse(read) => {
                    let output = read.resume(input.take())?;
                    let bytes = output.bytes();

                    match memrchr(b'\n', bytes) {
                        Some(n) => {
                            self.response.extend(&bytes[..n]);
                            break Ok(serde_json::from_slice(&self.response).unwrap());
                        }
                        None => {
                            self.response.extend(bytes);
                            read.set_buffer(output.buffer);
                            continue;
                        }
                    }
                }
            }
        }
    }
}
