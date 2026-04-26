cfg_select! {
	target_os = "linux" => {
		mod linux;
		pub use linux::{host, vhci, socket, error::PlatformError};
	}
	_ => {
		mod unsupported;
		pub use unsupported::*;
	}
}
