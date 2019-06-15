use std::io;

pub type PacketSeq = u8;

pub trait Sequencer {
    fn next(&self) -> PacketSeq;
    fn is_valid(&self) -> bool;
}

impl Sequencer for PacketSeq {
    fn next(&self) -> PacketSeq {
        let n: u8 = *self;
        if n >= 0xef {
            1
        } else {
            n + 1
        }
    }

    fn is_valid(&self) -> bool {
        let n: u8 = *self;
        n > 0 && n < 0xf0
    }
}

pub const PACKET_DATA_BUF_LEN: usize = 128;

pub struct Packet {
    pub seq: PacketSeq,
    pub code: u8,
    pub data: Vec<u8>,
}

impl Packet {
    pub fn new() -> Self {
        Packet {
            seq: 0,
            code: 0,
            data: Vec::with_capacity(PACKET_DATA_BUF_LEN),
        }
    }

    pub fn new_with_seq(seq: u8) -> Self {
        Packet::new_with(seq, 0)
    }

    pub fn new_with(seq: u8, code: u8) -> Self {
        Packet {
            seq,
            code,
            data: Vec::with_capacity(PACKET_DATA_BUF_LEN),
        }
    }

    pub fn encode<W: io::Write>(&self, w: &mut W) -> io::Result<usize> {
        let mut head: [u8; 3] = [self.seq, self.code & 0x8f, self.data.len() as u8];
        let mut count = w.write(if head[2] < 7 {
            head[1] |= (head[2] << 4) & 0x70;
            &head[..2]
        } else {
            head[1] |= 0x70;
            &head[..]
        })?;
        if self.data.len() > 0 {
            count += w.write(self.data.as_slice())?;
        }
        Ok(count)
    }
}
