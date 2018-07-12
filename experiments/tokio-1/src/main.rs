extern crate tokio;
#[macro_use]
extern crate futures;
extern crate bytes;


use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use bytes::BytesMut;


fn main() {
	let addr: std::net::SocketAddr = "0.0.0.0:8080".parse().unwrap();

	let listener = TcpListener::bind(&addr).unwrap();

	let server = listener.incoming().for_each(move |socket| {
		process(socket);
		Ok(())
	})
	.map_err(|err| eprintln!("accept error = {:?}", err));

	println!("Server running on {}", addr);

	tokio::run(server);
}


fn process(socket: TcpStream) {
	let (reader, writer) = socket.split();

	let connection = Connection {
		socket: reader,
		buffer: BytesMut::new(),
		scan_pos: 0,
		line_pos: 0,
	}
	.map_err(|err| {
		eprintln!("connection error: {}", err)
	})
	.fold(writer, |writer, _| {
		let amt = tokio::io::write_all(writer, &b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 13\r\n\r\nHello, World!"[..]);
		let amt = amt.map(|(writer, _)| writer);
		amt.map_err(|err| {
			eprintln!("connection error: {}", err)
		})
	})
	.map(|_| {
		println!("connection closed");
		()
	});

	tokio::spawn(connection);
}

struct Connection<S> {
	socket: S,
	buffer: BytesMut,
	scan_pos: usize,
	line_pos: usize,
}

impl<S: AsyncRead> Connection<S> {
	fn fill_read_buf(&mut self) -> Poll<(), tokio::io::Error> {
		loop {
			self.buffer.reserve(1024);
			let n = try_ready!(self.socket.read_buf(&mut self.buffer));

			if n == 0 {
				return Ok(Async::Ready(()));
			}
		}
	}
}

impl<S: AsyncRead> Stream for Connection<S> {
	type Item = ();
	type Error = tokio::io::Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		let sock_closed = self.fill_read_buf()?.is_ready();
		if sock_closed {
			return Ok(Async::Ready(None));
		}

		loop {
			if let Some(newline) = self.buffer[self.scan_pos..].iter().position(|&x| x == b'\n') {
				let empty_line = {
					let mut line = &self.buffer[self.line_pos..self.scan_pos+newline];
					if line[line.len() - 1] == b'\r' {
						line = &line[..line.len()-1];
					}
					//let line = std::str::from_utf8(line).unwrap();

					//println!("Received line: `{}`", line);

					line.len() == 0
				};

				self.buffer.advance(self.scan_pos + newline + 1);
				self.line_pos = 0;
				self.scan_pos = 0;

				if empty_line {
					return Ok(Async::Ready(Some(())));
				}
			}
			else {
				self.scan_pos = self.buffer.len();
				break;
			}
		}

		Ok(Async::NotReady)
	}
}