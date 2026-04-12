use std::net::{Ipv4Addr, SocketAddrV4};

use busman::{
	connection::Connection,
	interop::VhciDriver,
	protocol::{
		Frame, PayloadReplyDeviceImport, PayloadReplyDeviceList, PayloadRequestDeviceImport,
		PayloadRequestDeviceList,
	},
	result::Result,
};

fn main() -> Result<()> {
	let port = 9000;
	let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
	let mut conn = Connection::client(addr)?;

	conn.send(&Frame::RequestDeviceList(PayloadRequestDeviceList {}))?;

	let devices = match conn.recv()? {
		Frame::ReplyDeviceList(PayloadReplyDeviceList { status, devices }) => devices,
		other => {
			eprintln!("unexpected frame during USBIP handshake: {other:?}");
			todo!()
		}
	};

	println!("{devices:?}");

	conn.send(&Frame::RequestDeviceImport(PayloadRequestDeviceImport {
		bus_id: String::new(),
	}))?;

	let import_result = match conn.recv()? {
		Frame::ReplyDeviceImport(PayloadReplyDeviceImport {
			status: status_code,
			device,
		}) => {
			if status_code == 0 {
				device
			} else {
				todo!()
			}
		}
		other => {
			eprintln!("unexpected frame during USBIP handshake: {other:?}");
			todo!()
		}
	};

	// let driver = VhciDriver::init()?;
	// driver.export(socket, device)?;
	Ok(())
}
