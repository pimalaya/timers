//! # Response
//!
//! When a server receives a request, it sends back a response. This
//! module contains the response structure as well as traits to read
//! and write a response.

use serde::{Deserialize, Serialize};

use crate::timer::Timer;

/// The server response struct.
///
/// Responses are sent by servers and received by clients.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Response {
    /// Default response when everything goes as expected.
    Ok,

    /// Response containing the current timer.
    Timer(Timer),
}

impl Response {
    pub fn to_vec(&self) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(self).unwrap();
        bytes.push(b'\n');
        bytes
    }
}
