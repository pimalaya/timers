use std::mem;

use io_stream::{
    coroutines::{Read, Write},
    Io,
};
use log::debug;
use memchr::memrchr;

use crate::{timer::TimerEvent, Request, Response, Timer};

#[derive(Debug)]
pub enum State {
    ReceiveRequest(Read),
    SendResponse(Write),
}

#[derive(Debug)]
pub struct HandleRequest {
    state: State,
    request: Vec<u8>,
    events: Vec<TimerEvent>,
}

impl HandleRequest {
    pub fn new() -> Self {
        Self {
            state: State::ReceiveRequest(Read::default()),
            request: Vec::new(),
            events: Vec::with_capacity(2),
        }
    }

    pub fn resume(
        &mut self,
        timer: &mut Timer,
        mut io: Option<Io>,
    ) -> Result<impl IntoIterator<Item = TimerEvent>, Io> {
        loop {
            match &mut self.state {
                State::ReceiveRequest(read) => {
                    let output = read.resume(io.take())?;
                    let bytes = output.bytes();

                    let request = match memrchr(b'\n', bytes) {
                        Some(n) => {
                            self.request.extend(&bytes[..n]);
                            serde_json::from_slice(&self.request).unwrap()
                        }
                        None => {
                            self.request.extend(bytes);
                            read.set_buffer(output.buffer);
                            continue;
                        }
                    };

                    let response = match request {
                        Request::Start => {
                            debug!("start timer");
                            self.events.extend(timer.start());
                            Response::Ok
                        }
                        Request::Get => {
                            debug!("get timer");
                            Response::Timer(timer.clone())
                        }
                        Request::Set(duration) => {
                            debug!("set timer");
                            self.events.extend(timer.set(duration));
                            Response::Ok
                        }
                        Request::Pause => {
                            debug!("pause timer");
                            timer.pause();
                            Response::Ok
                        }
                        Request::Resume => {
                            debug!("resume timer");
                            self.events.extend(timer.resume());
                            Response::Ok
                        }
                        Request::Stop => {
                            debug!("stop timer");
                            self.events.extend(timer.stop());
                            Response::Ok
                        }
                    };

                    let coroutine = Write::new(response.to_vec());
                    self.state = State::SendResponse(coroutine);
                }
                State::SendResponse(write) => {
                    write.resume(io.take())?;
                    break Ok(mem::take(&mut self.events));
                }
            }
        }
    }
}
