use libc::{c_int, c_void, socklen_t};
use std::{net::TcpStream, os::fd::AsRawFd};

use crate::result::Result;

const ENABLE_TCP_KEEPALIVE: c_int = 1;
const TCP_KEEPALIVE_IDLE_DELAY_SECS: c_int = 5;
const TCP_KEEPALIVE_RETRY_INTERVAL_SECS: c_int = 2;
const TCP_MAX_FAILED_PROBES: c_int = 3;
const TCP_TIMEOUT_MS: c_int = 10000;

pub fn configure_socket_for_handoff(socket: &TcpStream) -> Result<()> {
	socket_enable_keepalive(socket)?;
	socket_set_keepalive_delay(socket)?;
	socket_set_keepalive_interval(socket)?;
	socket_set_max_keepalive_probes(socket)?;
	socket_set_timeout(socket)?;
	Ok(())
}

fn socket_enable_keepalive(socket: &TcpStream) -> Result<()> {
	set_socket_option(
		socket,
		libc::SOL_SOCKET, // level = SOL_SOCKET means that the option exists at the socket level
		libc::SO_KEEPALIVE,
		ENABLE_TCP_KEEPALIVE,
	)
}

fn socket_set_keepalive_delay(socket: &TcpStream) -> Result<()> {
	set_socket_option(
		socket,
		libc::SOL_TCP, // level = SOL_TCP means the option lives at the TCP protocol level
		libc::TCP_KEEPIDLE,
		TCP_KEEPALIVE_IDLE_DELAY_SECS,
	)
}

fn socket_set_keepalive_interval(socket: &TcpStream) -> Result<()> {
	set_socket_option(
		socket,
		libc::SOL_TCP,
		libc::TCP_KEEPINTVL,
		TCP_KEEPALIVE_RETRY_INTERVAL_SECS,
	)
}

fn socket_set_max_keepalive_probes(socket: &TcpStream) -> Result<()> {
	set_socket_option(socket, libc::SOL_TCP, libc::TCP_KEEPCNT, TCP_MAX_FAILED_PROBES)
}

fn socket_set_timeout(socket: &TcpStream) -> Result<()> {
	set_socket_option(socket, libc::SOL_TCP, libc::TCP_USER_TIMEOUT, TCP_TIMEOUT_MS)
}

fn set_socket_option(socket: &TcpStream, level: c_int, name: c_int, value: c_int) -> Result<()> {
	let fd = socket.as_raw_fd();
	let c_int_size = std::mem::size_of::<c_int>() as socklen_t;

	// Syscall param is a const void pointer, need to first cast to i32 pointer
	let value = &value as *const c_int as *const c_void;

	match unsafe { libc::setsockopt(fd, level, name, value, c_int_size) } {
		0 => Ok(()),
		_ => Err(std::io::Error::last_os_error().into()),
	}
}
