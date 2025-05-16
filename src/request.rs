//! # Request
//!
//! To control the timer, a client sends requests to the server and
//! receive back a response. This module contains the request
//! structure as well as trait to read and write a request.

use serde::{Deserialize, Serialize};

/// The client request struct.
///
/// Requests are sent by clients and received by servers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Request {
    /// Request the timer to start with the first configured cycle.
    Start,

    /// Request the state, the cycle and the value of the timer.
    Get,

    /// Request to change the current timer duration.
    Set(usize),

    /// Request to pause the timer.
    ///
    /// A paused timer freezes, which means it keeps its state, cycle
    /// and value till it get resumed.
    Pause,

    /// Request to resume the paused timer.
    ///
    /// Has no effect if the timer is not paused.
    Resume,

    /// Request to stop the timer.
    ///
    /// Stopping the timer resets the state, the cycle and the value.
    Stop,
}

impl Request {
    pub fn to_vec(&self) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(self).unwrap();
        bytes.push(b'\n');
        bytes
    }
}
