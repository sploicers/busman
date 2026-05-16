use crate::{
	platform::socket::configure_socket_for_handoff,
	protocol::{Decode, Encode, Frame},
	result::{DecodeResult, EncodeResult, Result},
};
use std::{
	net::{SocketAddrV4, TcpListener, TcpStream},
	os::fd::AsRawFd,
};

pub struct Connection {
	socket: TcpStream,
}

pub struct Server {
	listener: TcpListener,
}

impl Server {
	pub fn listen(addr: SocketAddrV4) -> Result<Self> {
		let listener = TcpListener::bind(addr)?;
		Ok(Self { listener })
	}

	pub fn accept(&self) -> Result<Connection> {
		let (socket, addr) = self.listener.accept()?;
		log::debug!("Accepted connection from {addr}.");
		Connection::server(socket)
	}
}

impl Connection {
	pub fn client(addr: SocketAddrV4) -> Result<Self> {
		log::debug!("Initiating connection to {addr}...");
		let socket = TcpStream::connect(addr)?;
		configure_socket_for_handoff(&socket)?;
		log::debug!("Connection established.");
		Ok(Self { socket })
	}

	pub fn server(socket: TcpStream) -> Result<Self> {
		configure_socket_for_handoff(&socket)?;
		Ok(Self { socket })
	}

	pub fn send(&mut self, frame: &Frame) -> EncodeResult<()> {
		log::debug!("SEND: {frame}");
		frame.encode(&mut self.socket)
	}

	pub fn recv(&mut self) -> DecodeResult<Frame> {
		let frame = Frame::decode(&mut self.socket)?;
		log::debug!("RECV: {frame}");
		Ok(frame)
	}

	pub fn into_socket(self) -> TcpStream {
		self.socket
	}

	pub fn socket_fd(&self) -> i32 {
		self.socket.as_raw_fd()
	}
}
