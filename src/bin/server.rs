use std::{
	net::{Ipv4Addr, SocketAddrV4},
	thread::{self, JoinHandle},
};

use busman::{
	connection::{Connection, Server},
	interop::{HostDriver, list_devices},
	protocol::{
		Frame, PayloadReplyDeviceImport, PayloadReplyDeviceList, PayloadRequestDeviceImport,
		USBDevice,
	},
	result::Result,
};

fn main() -> Result<()> {
	let port = 9000;
	let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
	let server = Server::listen(addr)?;
	let mut threads = vec![];

	loop {
		let mut conn = server.accept()?;

		// Every time a connection is accepted, spawn thread and attempt initial USBIP handshake
		let handler: JoinHandle<Result<()>> = thread::spawn(move || {
			loop {
				match conn.recv()? {
					Frame::RequestDeviceList(_) => handle_device_list_request(&mut conn)?,
					Frame::RequestDeviceImport(PayloadRequestDeviceImport { bus_id }) => {
						handle_device_import_request(&mut conn, bus_id)?;
						break;
					}
					_ => {}
				};
			}
			Ok(())
		});

		threads.push(handler); // TODO: attempt join of all threads on SIGINT/SIGTERM
	}
}

fn handle_device_list_request(conn: &mut Connection) -> Result<()> {
	conn.send(&Frame::ReplyDeviceList(PayloadReplyDeviceList {
		status: 0,
		devices: list_devices()?,
	}))?;
	Ok(())
}

fn handle_device_import_request(conn: &mut Connection, bus_id: String) -> Result<()> {
	conn.send(&Frame::ReplyDeviceImport(PayloadReplyDeviceImport {
		status: 0,
		device: Some(USBDevice::default()),
	}))?;
	Ok(())
}
