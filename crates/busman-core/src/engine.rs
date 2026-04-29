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
	protocol::{
		Frame, PayloadReplyDeviceImport, PayloadReplyDeviceList, PayloadRequestDeviceImport, PayloadRequestDeviceList,
		USBDevice,
	},
	result::Result,
};

pub struct Engine<'a> {
	sender: Sender<Event>,
	events: Box<dyn Iterator<Item = Event> + 'a>,
	client: Option<Connection>,
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
			client: None,
			server: None,
		})
	}

	pub fn client_connect(&mut self, addr: SocketAddrV4) -> Result<()> {
		self.client = Some(Connection::client(addr)?);
		Ok(())
	}

	pub fn start_server(&mut self, addr: SocketAddrV4) {
		let host = self.host.clone();
		let sender = self.sender.clone();

		let handle: JoinHandle<Result<()>> = thread::spawn(move || {
			let mut threads = vec![];
			let server = Server::listen(addr)?;
			_ = sender.send(Event::ServerStart(addr));

			loop {
				let mut conn = server.accept()?;
				let host = host.clone();
				_ = sender.send(Event::ConnectionAccepted(addr));

				threads.push(thread::spawn(move || {
					loop {
						match conn.recv()? {
							Frame::RequestDeviceList(_) => handle_device_list_request(&mut conn, &host)?,
							Frame::RequestDeviceImport(PayloadRequestDeviceImport { bus_id }) => {
								return handle_device_import_request(conn, &host, bus_id);
							}
							_ => {}
						}
					}
				}));
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

	pub fn query_devices(&mut self) -> Result<Vec<USBDevice>> {
		Ok(if let Some(conn) = &mut self.client {
			conn.send(&Frame::RequestDeviceList(PayloadRequestDeviceList {}))?;

			match conn.recv()? {
				Frame::ReplyDeviceList(PayloadReplyDeviceList { devices, .. }) => devices,
				other => {
					log::error!("unexpected frame during USBIP handshake: {other:?}");
					vec![]
				}
			}
		} else {
			vec![]
		})
	}

	pub fn import_device(&mut self, bus_id: &str) -> Result<()> {
		if let Some(client) = &mut self.client {
			client.send(&Frame::RequestDeviceImport(PayloadRequestDeviceImport {
				bus_id: bus_id.to_owned(),
			}))?;

			match client.recv()? {
				Frame::ReplyDeviceImport(PayloadReplyDeviceImport {
					status: 0,
					device: Some(device),
				}) => {
					log::info!("Successfully imported device {device:?}")
				}
				Frame::ReplyDeviceImport(PayloadReplyDeviceImport { status: 1, .. }) => {
					log::error!("Non-zero status code when attempting to import device {bus_id}")
				}
				other => {
					log::error!("unexpected frame during USBIP handshake: {other:?}");
				}
			};
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
