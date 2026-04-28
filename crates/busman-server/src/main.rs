use std::net::{Ipv4Addr, SocketAddrV4};

use busman_core::{engine::Engine, result::Result};

fn main() -> Result<()> {
	env_logger::init();
	let port = 9000;
	let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
	log::info!("Initializing...");

	let mut engine = Engine::new()?;
	engine.start_server(addr);

	for event in engine {
		log::info!("{event}")
	}
	Ok(())
}
