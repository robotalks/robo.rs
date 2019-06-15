#![cfg(test)]

use std::fmt;
use super::*;

impl PartialEq for Packet {
    fn eq(&self, other: &Self) -> bool {
        self.seq == other.seq && self.code == other.code && self.data == other.data
    }
}

impl PartialEq for ParseResult {
    fn eq(&self, other: &Self) -> bool {
        self.sync == other.sync && self.state == other.state && match &self.packet {
            Some(pkt) => match &other.packet {
                Some(pkt1) => pkt == pkt1,
                _ => false
            },
            None => match &other.packet {
                None => true,
                _ => false
            }
        }
    }
}

impl fmt::Debug for Packet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Packet {{ seq: {}, code: {}, data: {:?} }}", self.seq, self.code, self.data)
    }
}

impl fmt::Debug for ParseResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ParseResult {{ sync: {}, state: {}, packet: {} }}", 
            self.sync, self.state, match &self.packet {
                Some(pkt) => format!("{:?}", pkt),
                None => String::from("None")
            })
    }
}

macro_rules! test_packet_encode {
    ($name:ident, $pkt:expr, $($exp:expr),+) => {
        #[test]
        fn $name() {
            let pkt = $pkt;
            let expected: Vec<u8> = vec![$($exp),*];
            let mut w: Vec<u8> = Vec::new();
            assert_eq!(pkt.encode(&mut w).unwrap(), expected.len());
            assert_eq!(w.as_slice(), expected.as_slice());
        }
    }
}

test_packet_encode!(test_packet_encode_no_data, Packet::new_with(1, 2), 1, 2);
test_packet_encode!(test_packet_encode_small_data, Packet{seq: 1, code: 2, data: vec![1]}, 1, 0x12, 1);
test_packet_encode!(test_packet_encode_large_data, Packet{seq: 1, code: 2, data: vec![1, 2, 3, 4, 5, 6, 7]}, 1, 0x72, 7, 1, 2, 3, 4, 5, 6, 7);
test_packet_encode!(test_packet_encode_event_no_data, Packet::new_with(1, 0x82), 1, 0x82);
test_packet_encode!(test_packet_encode_event_small_data, Packet{seq: 1, code: 0x82, data: vec![1]}, 1, 0x92, 1);
test_packet_encode!(test_packet_encode_event_large_data, Packet{seq: 1, code: 0x82, data: vec![1, 2, 3, 4, 5, 6, 7]}, 1, 0xf2, 7, 1, 2, 3, 4, 5, 6, 7);

#[test]
fn test_packet_seq() {
    for n in 0xf0..0x100 {
        let s = n as u8;
        assert!(!s.is_valid());
        assert_eq!(s.next(), 1u8);
    }
    for n in 1u8..0xf0 {
        assert!(n.is_valid());
        if n == 0xef {
            assert_eq!(n.next(), 1u8);
        } else {
            assert_eq!(n.next(), n+1);
        }
    }
    let s: PacketSeq = 0;
    assert!(!s.is_valid());
    assert_eq!(s.next(), 1u8);
}

#[test]
fn test_parser_reset() {
    let mut p = Parser::new();
    let pr = p.reset();
    assert_eq!(pr.sync, SYNC_REQ);
    assert_eq!(pr.state, 0);
    assert!(pr.packet.is_none());
}

#[test]
fn test_sync_state() {
    assert!(!0u8.is_ready());
    assert!(!0u8.is_receiving());
    assert!(SYNC_STATE_READY.is_ready());
    assert!(!SYNC_STATE_READY.is_receiving());
    assert!(!SYNC_STATE_RECV.is_ready());
    assert!(SYNC_STATE_RECV.is_receiving());
    assert!((SYNC_STATE_READY|SYNC_STATE_RECV).is_ready());
    assert!((SYNC_STATE_READY|SYNC_STATE_RECV).is_receiving());
}

macro_rules! test_parse_result {
    ($name:ident, $pr:expr, $exp:expr) => {
        #[test]
        fn $name() {
            let pr: ParseResult = $pr;
            let expected: TimerAction = $exp;
            assert_eq!(pr.timer_action(), expected);
        }
    }
}

test_parse_result!(test_parse_result_sync, ParseResult{sync: 0, state:0, packet: None}, TimerAction::NoChange);
test_parse_result!(test_parse_result_sync_req, ParseResult{sync: SYNC_REQ, state: 0, packet: None}, TimerAction::Restart);
test_parse_result!(test_parse_result_sync_ack, ParseResult{sync: SYNC_ACK, state: 0, packet: None}, TimerAction::NoChange);
test_parse_result!(test_parse_result_recv, ParseResult{sync: 0, state: SYNC_STATE_RECV, packet: None}, TimerAction::Restart);
test_parse_result!(test_parse_result_ready, ParseResult{sync: 0, state: SYNC_STATE_READY, packet: None}, TimerAction::Stop);
test_parse_result!(test_parse_result_ready_ack, ParseResult{sync: SYNC_ACK, state: SYNC_STATE_READY, packet: None}, TimerAction::Stop);

struct ParserTestSeq {
    input: Vec<u8>,
    expect: ParseResult,
    last: ParseResult,
}

impl ParserTestSeq {
    fn new() -> ParserTestSeq {
        ParserTestSeq {
            input: Vec::new(),
            expect: ParseResult{sync: 0, state: 0, packet: None},
            last: ParseResult{sync: 0, state: 0, packet: None},
        }
    }

    fn parse(mut self, input: &[u8]) -> Self {
        let v = &mut self.input;
        v.extend_from_slice(input);
        self
    }

    fn expect(mut self, pr: ParseResult) -> Self {
        self.expect = pr;
        self
    }

    fn expect_syncing(self) -> Self {
        self.expect(ParseResult{sync: 0, state: SYNC_STATE_RECV, packet: None})
    }

    fn expect_receiving(self) -> Self {
        self.expect(ParseResult{sync: 0, state: SYNC_STATE_READY|SYNC_STATE_RECV, packet: None})
    }

    fn expect_last(mut self, pr: ParseResult) -> Self {
        self.last = pr;
        self
    }

    fn syncing(self) -> Self {
        self.expect_last(ParseResult{sync: 0, state: SYNC_STATE_RECV, packet: None})
    }

    fn resync(self) -> Self {
        self.expect_last(ParseResult{sync: SYNC_REQ, state: 0, packet: None})
    }

    fn synced(self) -> Self {
        self.expect_last(ParseResult{sync: 0, state: SYNC_STATE_READY, packet: None})
    }

    fn synced_with_ack(self) -> Self {
        self.expect_last(ParseResult{sync: SYNC_ACK, state: SYNC_STATE_READY, packet: None})
    }

    fn packet(self, seq: u8, code: u8, data: &[u8]) -> Self {
        self.expect_last(ParseResult{sync: 0, state: SYNC_STATE_READY, packet: Some(Packet{seq, code, data: Vec::from(data)})})
    }
}

fn test_parser(seqs: &[ParserTestSeq]) {
    let mut p = Parser::new();
    for seq in seqs {
        let l = seq.input.len();
        let pr = if l > 0 {
            for i in 0..l-1 {
                assert_eq!(p.parse(seq.input[i]), seq.expect);
            }
            p.parse(seq.input[l-1])
        } else {
            p.timeout()
        };
        assert_eq!(pr, seq.last);
    }
}

macro_rules! test_parser {
    ($name:ident, $($seq:expr),+) => {
        #[test]
        fn $name() {
            test_parser(&[$($seq),*]);
        }
    }
}

macro_rules! parse {
    ($($b:expr),+) => {
        ParserTestSeq::new().parse(&[$($b),*])
    }
}

macro_rules! timeout {
    () => {
        ParserTestSeq::new()
    }
}

test_parser!(test_parser_sync_and_recv,
    parse!(SYNC_ACK, 1).expect_syncing().synced(),
    parse!(1, 2).expect_receiving().packet(1, 2, &[]),
    parse!(2, 0x72, 0).expect_receiving().packet(2, 2, &[]),
    parse!(3, 0x92, 3).expect_receiving().packet(3, 0x82, &[3]),
    parse!(4, 0x72, 0x08, 1, 2, 3, 4, 5, 6, 7, 8).expect_receiving().packet(4, 2, &[1, 2, 3, 4, 5, 6, 7, 8])
);

test_parser!(test_parser_sync_timeout,
    timeout!().resync(),
    parse!(SYNC_ACK).syncing(),
    timeout!().resync()
);

test_parser!(test_parser_sync_skip_invalid_bytes,
    parse!(1, 2, 3, 4, 0x80, 0x81, 0xf0, 0xf1),
    parse!(SYNC_ACK, 1).expect_syncing().synced()
);

test_parser!(test_parser_handle_req_in_sync,
    parse!(SYNC_REQ, 1).expect_syncing().synced_with_ack()
);

test_parser!(test_parser_handle_req_in_sync_with_invalid_seq,
    parse!(SYNC_REQ, SYNC_REQ).expect_syncing().resync(),
    parse!(SYNC_ACK, 1).expect_syncing().synced()
);

test_parser!(test_parser_handle_req_after_sync,
    parse!(SYNC_ACK, 1).expect_syncing().synced(),
    parse!(SYNC_REQ, 1).expect_syncing().synced_with_ack(),
    parse!(1, 2).expect_receiving().packet(1, 2, &[])
);

test_parser!(test_parser_handle_req_after_sync_with_invalid_seq,
    parse!(SYNC_ACK, 1).expect_syncing().synced(),
    parse!(SYNC_REQ, SYNC_ACK).expect_syncing().resync(),
    parse!(SYNC_ACK, 1).expect_syncing().synced()
);

test_parser!(test_parser_handle_ack_in_sync_with_invalid_seq,
    parse!(SYNC_ACK, SYNC_REQ).expect_syncing().resync(),
    parse!(SYNC_ACK, 1).expect_syncing().synced()
);

test_parser!(test_parser_handle_ack_after_sync,
	parse!(SYNC_ACK, 1).expect_syncing().synced(),
    parse!(SYNC_ACK, 1).expect_receiving().synced(),
    parse!(1, 2).expect_receiving().packet(1, 2, &[])
);

test_parser!(test_parser_ack_invalid_seq_after_sync,
    parse!(SYNC_ACK, 1).expect_syncing().synced(),
	parse!(SYNC_ACK, 2).expect_receiving().resync(),
	parse!(SYNC_ACK, 2).expect_syncing().synced(),
	parse!(2, 2).expect_receiving().packet(2, 2, &[])
);

test_parser!(test_parser_invalid_seq,
    parse!(SYNC_ACK, 1).expect_syncing().synced(),
    parse!(1, 2).expect_receiving().packet(1, 2, &[]),
    parse!(1).expect_syncing().resync(),
    parse!(0x92, 3),
    parse!(SYNC_ACK, 3).expect_syncing().synced()
);

test_parser!(test_parser_invalid_data_len,
    parse!(SYNC_ACK, 1).expect_syncing().synced(),
	parse!(1, 0x70, 0x80).expect_receiving().resync(),
	parse!(1, 2, 3, 4),
    parse!(SYNC_ACK, 1).expect_syncing().synced()
);
