use std::{
	net::SocketAddrV4,
	sync::{
		Arc,
		mpsc::{self, Receiver, Sender},
	},
};

use crate::{
	connection::Connection,
	engine::events::Event,
	platform::vhci::Vhci,
	protocol::{
		Frame, PayloadReplyDeviceImport, PayloadReplyDeviceList, PayloadRequestDeviceImport, PayloadRequestDeviceList,
		USBDevice,
	},
	result::Result,
};

pub struct ClientEngine {
	client: Option<Connection>,
	vhci: Arc<Vhci>,
	tx: Sender<Event>,
	rx: Receiver<Event>,
}

impl ClientEngine {
	pub fn new() -> Result<Self> {
		let (tx, rx) = mpsc::channel();
		Ok(Self {
			client: None,
			vhci: Arc::new(Vhci::new()),
			tx,
			rx,
		})
	}

	pub fn client_connect(&mut self, addr: SocketAddrV4) -> Result<()> {
		self.client = Some(Connection::client(addr)?);
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

			let reply = client.recv()?;

			match reply {
				Frame::ReplyDeviceImport(PayloadReplyDeviceImport {
					status: 0,
					device: Some(device),
				}) => {
					self.vhci.import_device(&device, client.socket_fd())?;
					_ = self.tx.send(Event::ImportSuccess(device));
				}
				Frame::ReplyDeviceImport(PayloadReplyDeviceImport { status: 1, .. }) => {
					_ = self.tx.send(Event::Error(format!(
						"Non-zero status code when attempting to import device {bus_id}"
					)));
				}
				other => {
					_ = self.tx.send(Event::Error(format!(
						"unexpected frame during USBIP handshake: {other:?}"
					)));
				}
			};
		}
		Ok(())
	}
}

impl Iterator for ClientEngine {
	type Item = Event;

	fn next(&mut self) -> Option<Self::Item> {
		self.rx.recv().ok()
	}
}
