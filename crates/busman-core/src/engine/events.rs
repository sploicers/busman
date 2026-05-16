use std::{fmt::Display, net::SocketAddrV4};

use crate::protocol::USBDevice;

#[derive(Debug)]
pub enum Event {
	ServerStart(SocketAddrV4),
	ServerStop,
	ConnectionAccepted(SocketAddrV4),
	DeviceExportStart,
	DeviceExportStop,
	NewDevice,
	ImportSuccess(USBDevice),
	Error(String),
}

impl Display for Event {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Event::ServerStart(addr) => write!(f, "Server listening at {addr}"),
			Event::ConnectionAccepted(addr) => write!(f, "Accepted connection from {addr}"),
			Event::Error(reason) => write!(f, "Server error: {reason}"),
			other => write!(f, "{other:?}"),
		}
	}
}
