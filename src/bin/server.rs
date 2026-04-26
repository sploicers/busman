use std::{
	net::{Ipv4Addr, SocketAddrV4},
	sync::Arc,
	thread::{self},
};

use busman::{
	connection::{Connection, Server},
	platform::host::Host,
	protocol::{
		Frame, PayloadReplyDeviceImport, PayloadReplyDeviceList, PayloadRequestDeviceImport,
	},
	result::Result,
};

fn main() -> Result<()> {
	env_logger::init();

	let port = 9000;
	let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
	let server = Server::listen(addr)?;
	let usbip_host = Arc::new(Host::new()?);
	let mut threads = vec![];

	loop {
		let mut conn = server.accept()?;
		let host = usbip_host.clone();

		// Every time a connection is accepted, spawn thread and attempt initial USBIP handshake
		let handler = thread::spawn(move || {
			loop {
				match conn.recv()? {
					Frame::RequestDeviceList(_) => handle_device_list_request(&mut conn, &host)?,
					Frame::RequestDeviceImport(PayloadRequestDeviceImport { bus_id }) => {
						return handle_device_import_request(conn, &host, bus_id);
					}
					_ => {}
				}
			}
		});

		threads.push(handler); // TODO: attempt join of all threads on SIGINT/SIGTERM
	}
}

fn handle_device_list_request(conn: &mut Connection, host: &Arc<Host>) -> Result<()> {
	conn.send(&Frame::ReplyDeviceList(PayloadReplyDeviceList {
		status: 0,
		devices: host.list_devices()?,
	}))?;
	Ok(())
}

fn handle_device_import_request(
	mut conn: Connection,
	host: &Arc<Host>,
	bus_id: String,
) -> Result<()> {
	let device = host.device_by_id(&bus_id)?;
	let exists = device.is_some();

	conn.send(&Frame::ReplyDeviceImport(PayloadReplyDeviceImport {
		status: if exists { 0 } else { 1 },
		device: device.clone(),
	}))?;

	if exists {
		let export = host.export_device(conn, device.unwrap())?;
		export.run_to_completion()?;
	}
	Ok(())
}
