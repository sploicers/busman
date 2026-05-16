use std::{
	net::SocketAddrV4,
	sync::{
		Arc,
		mpsc::{Receiver, Sender, channel},
	},
	thread::{self, JoinHandle},
};

use crate::{
	connection::{Connection, Server},
	engine::Event,
	platform::host::Host,
	protocol::{Frame, PayloadReplyDeviceImport, PayloadReplyDeviceList, PayloadRequestDeviceImport},
	result::Result,
};

pub struct ServerEngine {
	server: Option<ServerHandle>,
	host: Arc<Host>,
	tx: Sender<Event>,
	rx: Receiver<Event>,
}

struct ServerHandle(JoinHandle<Result<()>>);

impl ServerEngine {
	pub fn new() -> Result<Self> {
		let (tx, rx) = channel::<Event>();

		Ok(Self {
			server: None,
			host: Arc::new(Host::new()?),
			tx,
			rx,
		})
	}

	pub fn start_server(&mut self, addr: SocketAddrV4) {
		let host = self.host.clone(); // These clones are both cheap because they're actually just refcount increments
		let sender = self.tx.clone();

		let handle = spawn_thread_and_forward_errors(sender.clone(), move || {
			let mut threads = vec![];
			let server = Server::listen(addr)?;
			_ = sender.send(Event::ServerStart(addr));

			loop {
				let mut conn = server.accept()?;
				let host = host.clone();
				_ = sender.send(Event::ConnectionAccepted(addr));

				threads.push(spawn_thread_and_forward_errors(sender.clone(), move || {
					loop {
						match conn.recv()? {
							Frame::RequestDeviceList(_) => handle_device_list_request(&mut conn, &host)?,
							Frame::RequestDeviceImport(PayloadRequestDeviceImport { bus_id }) => {
								return handle_device_import_request(conn, &host, bus_id);
							}
							_ => {}
						}
					}
				}))
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

impl Iterator for ServerEngine {
	type Item = Event;

	fn next(&mut self) -> Option<Self::Item> {
		self.rx.recv().ok()
	}
}

fn spawn_thread_and_forward_errors<F, T>(sender: Sender<Event>, f: F) -> JoinHandle<Result<T>>
where
	F: FnOnce() -> Result<T> + Send + 'static,
	T: Send + 'static,
{
	thread::spawn(move || {
		let result = f();
		if let Err(e) = &result {
			_ = sender.send(Event::Error(e.to_string()));
		}
		result
	})
}

fn handle_device_list_request(conn: &mut Connection, host: &Host) -> Result<()> {
	conn.send(&Frame::ReplyDeviceList(PayloadReplyDeviceList {
		status: 0,
		devices: host.list_devices()?,
	}))?;
	Ok(())
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
