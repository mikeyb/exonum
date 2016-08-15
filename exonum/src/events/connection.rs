use std::io;
use std::mem::swap;
use std::net::SocketAddr;
use std::collections::VecDeque;

use byteorder::{ByteOrder, LittleEndian};

use mio::tcp::TcpStream;
use mio::{TryWrite, TryRead};

use super::super::messages::{RawMessage, MessageBuffer, HEADER_SIZE};

#[derive(Debug, PartialEq)]
pub struct MessageReader {
    raw: Vec<u8>,
}

impl MessageReader {
    pub fn empty() -> MessageReader {
        MessageReader { raw: vec![0; HEADER_SIZE] }
    }

    pub fn actual_len(&self) -> usize {
        self.raw.len()
    }

    pub fn total_len(&self) -> usize {
        LittleEndian::read_u32(&self.raw[4..8]) as usize
    }

    pub fn allocate(&mut self) {
        let size = self.total_len();
        self.raw.resize(size, 0);
    }

    pub fn buffer(&mut self) -> &mut [u8] {
        &mut self.raw
    }

    pub fn into_raw(self) -> MessageBuffer {
        MessageBuffer::from_vec(self.raw)
    }
}

pub struct IncomingConnection {
    socket: TcpStream,
    // address: SocketAddr,
    raw: MessageReader,
    position: usize,
}

pub struct OutgoingConnection {
    socket: TcpStream,
    address: SocketAddr,
    queue: VecDeque<RawMessage>,
    position: usize,
}

impl IncomingConnection {
    pub fn new(socket: TcpStream /* , address: SocketAddr */) -> IncomingConnection {
        IncomingConnection {
            socket: socket,
            // address: address,
            raw: MessageReader::empty(),
            position: 0,
        }
    }

    pub fn socket(&self) -> &TcpStream {
        &self.socket
    }

    // pub fn  address(&self) -> &SocketAddr {
    //     &self.address
    // }

    fn read(&mut self) -> io::Result<Option<usize>> {
        // FIXME: we shouldn't read more than HEADER_SIZE or total_length()
        // TODO: read into growable Vec, not into [u8]
        if self.position == HEADER_SIZE && self.raw.actual_len() == HEADER_SIZE {
            self.raw.allocate();
        }
        self.socket.try_read(&mut self.raw.buffer()[self.position..])
    }

    pub fn readable(&mut self) -> io::Result<Option<MessageBuffer>> {
        // TODO: raw length == 0?
        // TODO: maximum raw length?
        loop {
            match self.read()? {
                None | Some(0) => return Ok(None),
                Some(n) => {
                    self.position += n;
                    if self.position >= HEADER_SIZE && self.position == self.raw.total_len() {
                        let mut raw = MessageReader::empty();
                        swap(&mut raw, &mut self.raw);
                        self.position = 0;
                        return Ok(Some(raw.into_raw()));
                    }
                }
            }
        }
    }
}

impl OutgoingConnection {
    pub fn new(socket: TcpStream, address: SocketAddr) -> OutgoingConnection {
        OutgoingConnection {
            socket: socket,
            address: address,
            queue: VecDeque::new(),
            position: 0,
        }
    }

    pub fn socket(&self) -> &TcpStream {
        &self.socket
    }

    pub fn address(&self) -> &SocketAddr {
        &self.address
    }

    pub fn writable(&mut self) -> io::Result<()> {
        // TODO: use try_write_buf
        while let Some(message) = self.queue.pop_front() {
            match self.socket.try_write(message.as_ref().as_ref())? {
                None | Some(0) => {
                    self.queue.push_front(message);
                    break;
                }
                Some(n) => {
                    // FIXME: What if we write less than message size?
                    self.position += n;
                    if n == message.len() {
                        self.position = 0;
                    }
                }
            }
        }
        // TODO: reregister
        Ok(())
    }

    pub fn send(&mut self, message: RawMessage) {
        // TODO: capacity overflow
        // TODO: reregister
        self.queue.push_back(message);
    }

    pub fn is_idle(&self) -> bool {
        self.queue.is_empty()
    }
}
