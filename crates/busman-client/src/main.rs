use busman_core::{engine::Engine, result::Result};

fn main() -> Result<()> {
	env_logger::init();
	log::info!("Initializing...");

	let mut engine = Engine::new()?;
	let devices = engine.query_devices()?;
	let selected_device = devices.first().expect("Must specify device to import");

	engine.import_device(&selected_device.bus_id)?;
	Ok(())
}
