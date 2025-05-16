#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(feature = "client")]
pub mod client;
mod request;
mod response;
#[cfg(feature = "server")]
pub mod server;
pub mod timer;

#[doc(inline)]
pub use self::{request::Request, response::Response, timer::Timer};
