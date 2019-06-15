use super::packet::*;

pub const SYNC_REQ: u8 = 0xff;
pub const SYNC_ACK: u8 = 0xfe;

pub type SyncState = u8;

pub const SYNC_STATE_READY: SyncState = 0x01;
pub const SYNC_STATE_RECV:  SyncState = 0x02;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerAction {
    NoChange,   // no change to current timer.
    Restart,    // restart the timer.
    Stop,       // stop the timer.
}

pub trait SyncStateReader {
    fn is_ready(self) -> bool;
    fn is_receiving(self) -> bool;
}

impl SyncStateReader for SyncState {
    fn is_ready(self) -> bool {
        self & SYNC_STATE_READY != 0
    }

    fn is_receiving(self) -> bool {
        self & SYNC_STATE_RECV != 0
    }
}

pub struct ParseResult {
    pub sync: u8,
    pub state: SyncState,
    pub packet: Option<Packet>,
}

impl ParseResult {
    fn new(sync: u8, state: SyncState) -> ParseResult {
        ParseResult {
            sync,
            state,
            packet: None,
        }
    }

    fn new_packet(pkt: Option<Packet>) -> ParseResult {
        ParseResult {
            sync: 0,
            state: SYNC_STATE_READY,
            packet: pkt,
        }
    }

    pub fn timer_action(&self) -> TimerAction {
        if self.state.is_receiving() || self.sync == SYNC_REQ {
            TimerAction::Restart
        } else if self.state.is_ready() {
            TimerAction::Stop
        } else {
            TimerAction::NoChange
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParsingState {
    SyncAck,    // sync req sent, waiting for syncACK
    SyncReqSeq, // waiting for sync seq after syncREQ
    SyncAckSeq, // waiting for sync seq after syncACK
    MsgSeq,     // waiting for message seq
    MsgAckSeq,  // recv syncACK in MsgSeq, wait seq
    MsgCode,    // waiting for message code
    MsgLen,     // waiting for message length
    MsgData,    // waiting for message data
}

pub struct Parser {
    state: ParsingState,
    peer_seq: PacketSeq,
    packet: Option<Packet>,
    data_len: usize,
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            state: ParsingState::SyncAck,
            peer_seq: 0,
            packet: None,
            data_len: 0,
        }
    }

    pub fn reset(&mut self) -> ParseResult {
        self.state = ParsingState::SyncAck;
        ParseResult::new(SYNC_REQ, 0)
    }

    pub fn parse(&mut self, b: u8) -> ParseResult {
        match self.state {
            ParsingState::SyncAck => match b {
                    SYNC_REQ => self.transit_and_result(ParsingState::SyncReqSeq),
                    SYNC_ACK => self.transit_and_result(ParsingState::SyncAckSeq),
                    _ => self.result_from_state()
                },
            ParsingState::SyncReqSeq => if b.is_valid() {
                    self.peer_seq = b;
                    self.state = ParsingState::MsgSeq;
                    ParseResult::new(SYNC_ACK, SYNC_STATE_READY)
                } else {
                    self.reset()
                },
            ParsingState::SyncAckSeq => if b.is_valid() {
                    self.peer_seq = b;
                    self.transit_and_result(ParsingState::MsgSeq)
                } else {
                    self.reset()
                },
            ParsingState::MsgSeq => match b {
                    SYNC_REQ => self.transit_and_result(ParsingState::SyncReqSeq),
                    SYNC_ACK => self.transit_and_result(ParsingState::MsgAckSeq),
                    b if b == self.peer_seq => {
                            self.packet.replace(Packet::new_with_seq(b));
                            self.peer_seq = self.peer_seq.next();
                            self.transit_and_result(ParsingState::MsgCode)
                        },
                    _ => self.reset()
                },
            ParsingState::MsgAckSeq => if b == self.peer_seq {
                    self.transit_and_result(ParsingState::MsgSeq)
                } else {
                    self.reset()
                },
            ParsingState::MsgCode => {
                let pkt = self.packet.as_mut().unwrap();
                pkt.code = b & 0x8f;
                let data_len = (b >> 4) & 7;
                match data_len {
                    0 => self.packet_ready(),
                    7 => self.transit_and_result(ParsingState::MsgLen),
                    _ => {
                        self.data_len = data_len as usize;
                        self.transit_and_result(ParsingState::MsgData)
                    }
                }
            },
            ParsingState::MsgLen => if b >= 0x80 {
                    self.reset()
                } else if b == 0 {
                    self.packet_ready()
                } else {
                    self.data_len = b as usize;
                    self.transit_and_result(ParsingState::MsgData)
                },
            ParsingState::MsgData => {
                let pkt = self.packet.as_mut().unwrap();
                pkt.data.push(b);
                if pkt.data.len() >= self.data_len {
                    self.packet_ready()
                } else {
                    self.result_from_state()
                }
            }
        }
    }

    pub fn timeout(&mut self) -> ParseResult {
        if self.state != ParsingState::MsgSeq {
            self.reset()
        } else {
            self.result_from_state()
        }
    }

    fn transit_and_result(&mut self, state: ParsingState) -> ParseResult {
        self.state = state;
        self.result_from_state()
    }

    fn result_from_state(&self) -> ParseResult {
        ParseResult::new(0, match self.state {
            ParsingState::SyncAck => 0,
            ParsingState::SyncReqSeq |
            ParsingState::SyncAckSeq => SYNC_STATE_RECV,
            ParsingState::MsgSeq => SYNC_STATE_READY,
            ParsingState::MsgAckSeq |
            ParsingState::MsgCode |
            ParsingState::MsgLen |
            ParsingState::MsgData => SYNC_STATE_READY | SYNC_STATE_RECV,
        })
    }

    fn packet_ready(&mut self) -> ParseResult {
        self.state = ParsingState::MsgSeq;
        ParseResult::new_packet(self.packet.take())
    }
}
