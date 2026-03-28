//! Debug Adapter Protocol (DAP) Module for Bengal
//! 
//! This module provides DAP support for debugging Bengal programs over TCP transport.

pub mod types;
pub mod transport;
pub mod session;
pub mod server;

pub use session::DebugSession;
