use crate::{
	protocol::{Decode, Encode, Frame},
	result::{DecodeResult, EncodeResult, Result},
};
use std::net::{SocketAddrV4, TcpListener, TcpStream};

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
		println!("Accepted connection from {addr}.");
		Ok(Connection::server(socket))
	}
}

impl Connection {
	pub fn client(addr: SocketAddrV4) -> Result<Self> {
		println!("Initiating connection to {addr}...");
		let socket = TcpStream::connect(addr)?;
		println!("Connection established.");
		Ok(Self { socket })
	}

	pub fn server(socket: TcpStream) -> Self {
		Self { socket }
	}

	pub fn send(&mut self, frame: &Frame) -> EncodeResult<()> {
		println!("SEND: {frame}");
		frame.encode(&mut self.socket)
	}

	pub fn recv(&mut self) -> DecodeResult<Frame> {
		let frame = Frame::decode(&mut self.socket)?;
		println!("RECV: {frame}");
		Ok(frame)
	}
}
