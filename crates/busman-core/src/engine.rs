use std::{
	fmt::Display,
	net::SocketAddrV4,
	sync::{
		Arc,
		mpsc::{Sender, channel},
	},
	thread::{self, JoinHandle},
};

use crate::{
	connection::{Connection, Server},
	platform::host::Host,
	protocol::{Frame, PayloadReplyDeviceImport, PayloadReplyDeviceList, PayloadRequestDeviceImport},
	result::Result,
};

pub struct Engine<'a> {
	sender: Sender<Event>,
	events: Box<dyn Iterator<Item = Event> + 'a>,
	server: Option<ServerHandle>,
	host: Arc<Host>,
}

#[derive(Debug)]
pub enum Event {
	ServerStart(SocketAddrV4),
	ServerStop,
	ConnectionAccepted(SocketAddrV4),
	DeviceExportStart,
	DeviceExportStop,
	NewDevice,
}

struct ServerHandle(JoinHandle<Result<()>>);

impl<'a> Engine<'a> {
	pub fn new() -> Result<Self> {
		let (tx, rx) = channel::<Event>();

		Ok(Self {
			sender: tx,
			events: Box::new(rx.into_iter()),
			host: Arc::new(Host::new()?),
			server: None,
		})
	}

	pub fn start_server(&mut self, addr: SocketAddrV4) {
		let host = self.host.clone();
		let sender = self.sender.clone();

		let handle: JoinHandle<Result<()>> = thread::spawn(move || {
			let server = Server::listen(addr)?;
			_ = sender.send(Event::ServerStart(addr));

			loop {
				let mut conn = server.accept()?;
				_ = sender.send(Event::ConnectionAccepted(addr));
				let host = host.clone();

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
			}
		});

		self.server = Some(ServerHandle(handle));
	}

	pub fn stop_server(&mut self) -> Result<()> {
		if let Some(handle) = self.server.take() {
			let _ = handle.0.join();
		}
		Ok(())
	}
}

impl<'a> Iterator for Engine<'a> {
	type Item = Event;

	fn next(&mut self) -> Option<Self::Item> {
		self.events.next()
	}
}

fn handle_device_list_request(conn: &mut Connection, host: &Host) -> Result<()> {
	conn.send(&Frame::ReplyDeviceList(PayloadReplyDeviceList {
		status: 0,
		devices: host.list_devices()?,
	}))?;
	Ok(())
}

impl Display for Event {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Event::ServerStart(addr) => write!(f, "Server listening at {addr}"),
			Event::ConnectionAccepted(addr) => write!(f, "Accepted connection from {addr}"),
			other => write!(f, "{other:?}"),
		}
	}
}

fn handle_device_import_request(mut conn: Connection, host: &Arc<Host>, bus_id: String) -> Result<()> {
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
