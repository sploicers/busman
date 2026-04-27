use std::net::{Ipv4Addr, SocketAddrV4};

use busman_core::{
	connection::Connection,
	protocol::{
		Frame, PayloadReplyDeviceImport, PayloadReplyDeviceList, PayloadRequestDeviceImport, PayloadRequestDeviceList,
	},
	result::Result,
};

fn main() -> Result<()> {
	env_logger::init();

	let port = 9000;
	let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
	let mut conn = Connection::client(addr)?;

	conn.send(&Frame::RequestDeviceList(PayloadRequestDeviceList {}))?;

	let devices = match conn.recv()? {
		Frame::ReplyDeviceList(PayloadReplyDeviceList { devices, .. }) => devices,
		other => {
			log::error!("unexpected frame during USBIP handshake: {other:?}");
			todo!("unexpected frame during USBIP handshake: {other:?}")
		}
	};

	let selected_device = devices.first().expect("Must specify device to import");

	conn.send(&Frame::RequestDeviceImport(PayloadRequestDeviceImport {
		bus_id: selected_device.bus_id.to_owned(),
	}))?;

	match conn.recv()? {
		Frame::ReplyDeviceImport(PayloadReplyDeviceImport {
			status: 0,
			device: Some(device),
		}) => {
			log::info!("Successfully imported device {device:?}")
		}
		Frame::ReplyDeviceImport(PayloadReplyDeviceImport { status: 1, .. }) => {
			log::error!("Non-zero status code when attempting to import device {selected_device:?}")
		}
		other => {
			log::error!("unexpected frame during USBIP handshake: {other:?}");
		}
	};
	Ok(())
}
