pub const MAX_PARAMS: usize = 32;
pub const MAX_OSC: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Ground,
    Escape,
    EscapeIntermediate,
    CsiEntry,
    CsiParam,
    CsiIntermediate,
    CsiIgnore,
    OscString,
    DcsEntry,
    DcsParam,
    DcsIntermediate,
    DcsPassthrough,
    DcsIgnore,
    SosPmApcString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Params {
    pub values: Vec<u16>,
    pub overflow: bool,
}

impl Params {
    fn new() -> Self {
        Self {
            values: Vec::new(),
            overflow: false,
        }
    }

    pub fn get(&self, index: usize) -> u16 {
        self.values.get(index).copied().unwrap_or(0)
    }

    pub fn get_or(&self, index: usize, default: u16) -> u16 {
        self.values.get(index).copied().filter(|v| *v != 0).unwrap_or(default)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Execute(u8),
    Print(u32),
    EscapeDispatch {
        intermediates: Vec<u8>,
        final_byte: u8,
    },
    CsiDispatch {
        private: Option<u8>,
        params: Params,
        intermediates: Vec<u8>,
        final_byte: u8,
    },
    OscDispatch {
        truncated: bool,
        payload: String,
    },
    DcsDispatch {
        final_byte: u8,
        params: Vec<u16>,
        payload: String,
    },
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Utf8State {
    Idle,
    Pending,
}

pub struct Parser {
    state: State,
    params: Params,
    current_param: Option<u16>,
    intermediates: Vec<u8>,
    private: Option<u8>,
    string: Vec<u8>,
    truncated: bool,
    dcs_final: Option<u8>,
    esc_string: Option<State>,
    utf8: [u8; 4],
    utf8_len: usize,
    utf8_need: usize,
    utf8_state: Utf8State,
    actions: Vec<Action>,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser {
    pub fn new() -> Self {
        Self {
            state: State::Ground,
            params: Params::new(),
            current_param: None,
            intermediates: Vec::new(),
            private: None,
            string: Vec::new(),
            truncated: false,
            dcs_final: None,
            esc_string: None,
            utf8: [0; 4],
            utf8_len: 0,
            utf8_need: 0,
            utf8_state: Utf8State::Idle,
            actions: Vec::new(),
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn take_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.actions)
    }

    pub fn reset(&mut self) {
        self.state = State::Ground;
        self.params = Params::new();
        self.current_param = None;
        self.intermediates.clear();
        self.private = None;
        self.string.clear();
        self.truncated = false;
        self.dcs_final = None;
        self.utf8_state = Utf8State::Idle;
        self.utf8_len = 0;
        self.utf8_need = 0;
    }

    pub fn advance(&mut self, bytes: &[u8]) -> Vec<Action> {
        for b in bytes {
            self.feed(*b);
        }
        self.take_actions()
    }

    /// Flushes an incomplete UTF-8 sequence at a true end of stream.
    ///
    /// Deliberately separate from `advance`: a multi-byte scalar routinely spans two
    /// chunks, so flushing after every call would corrupt every such character.
    pub fn flush(&mut self) -> Vec<Action> {
        self.flush_utf8();
        self.take_actions()
    }

    fn feed(&mut self, b: u8) {
        if self.state == State::Ground {
            self.ground(b);
        } else {
            self.non_ground(b);
        }
    }

    fn ground(&mut self, b: u8) {
        if self.utf8_state != Utf8State::Idle {
            self.utf8_byte(b);
            return;
        }
        match b {
            0x00..=0x17 | 0x19 | 0x1C..=0x1F => self.actions.push(Action::Execute(b)),
            0x18 | 0x1A => {
                self.abort();
            }
            0x1B => {
                self.clear_sequence();
                self.state = State::Escape;
            }
            0x20..=0x7F => self.actions.push(Action::Print(u32::from(b))),
            0x80..=0x9F => self.c1(b),
            _ => self.utf8_byte(b),
        }
    }

    fn c1(&mut self, b: u8) {
        match b {
            0x90 => {
                self.clear_sequence();
                self.state = State::DcsEntry;
            }
            0x9B => {
                self.clear_sequence();
                self.state = State::CsiEntry;
            }
            0x9D => {
                self.clear_sequence();
                self.state = State::OscString;
            }
            0x98 | 0x9E | 0x9F => {
                self.clear_sequence();
                self.state = State::SosPmApcString;
            }
            0x9C => {
                self.clear_sequence();
            }
            _ => self.actions.push(Action::Execute(b)),
        }
    }

    fn utf8_byte(&mut self, b: u8) {
        match self.utf8_state {
            Utf8State::Idle => {
                if b < 0x80 {
                    self.actions.push(Action::Print(u32::from(b)));
} else if (0xC2..=0xDF).contains(&b) {
                    self.begin_utf8(2);
                    self.utf8[0] = b;
                } else if (0xE0..=0xEF).contains(&b) {
                    self.begin_utf8(3);
                    self.utf8[0] = b;
                } else if (0xF0..=0xF4).contains(&b) {
                    self.begin_utf8(4);
                    self.utf8[0] = b;
                } else {
                    self.actions.push(Action::Print(0xFFFD));
                }
            }
            Utf8State::Pending => {
                if b & 0xC0 == 0x80 {
                    self.utf8[self.utf8_len] = b;
                    self.utf8_len += 1;
                    if self.utf8_len == self.utf8_need {
                        self.emit_utf8();
                    }
                } else {
                    self.actions.push(Action::Print(0xFFFD));
                    self.utf8_state = Utf8State::Idle;
                    self.utf8_len = 0;
                    self.ground(b);
}
            }
        }
    }

    fn begin_utf8(&mut self, need: usize) {
        self.utf8_need = need;
        self.utf8_len = 1;
        self.utf8_state = Utf8State::Pending;
    }

    fn emit_utf8(&mut self) {
        let n = self.utf8_len;
        let mut buf = [0u8; 4];
        buf[..n].copy_from_slice(&self.utf8[..n]);
        self.utf8_state = Utf8State::Idle;
        self.utf8_len = 0;
        match std::str::from_utf8(&buf[..n]) {
            Ok(s) => {
                if let Some(c) = s.chars().next() {
                    self.actions.push(Action::Print(u32::from(c)));
                }
            }
            Err(_) => self.actions.push(Action::Print(0xFFFD)),
        }
    }

    fn flush_utf8(&mut self) {
        if self.utf8_state == Utf8State::Pending {
            self.actions.push(Action::Print(0xFFFD));
            self.utf8_state = Utf8State::Idle;
            self.utf8_len = 0;
        }
    }

    fn non_ground(&mut self, b: u8) {
        if matches!(
            self.state,
            State::OscString | State::DcsPassthrough | State::SosPmApcString
        ) {
            match b {
                0x07 if self.state == State::OscString => self.finish_string(),
                0x1B => {
                    self.esc_string = Some(self.state);
                    self.state = State::Escape;
                }
                0x9C => self.finish_string(),
                0x18 | 0x1A => self.abort(),
                _ => self.string_byte(b),
            }
            return;
        }
        match b {
            0x18 | 0x1A => {
                self.abort();
            }
            0x1B => {
                self.clear_sequence();
                self.state = State::Escape;
            }
            0x9C => {
                self.clear_sequence();
                self.state = State::Ground;
            }
            0x00..=0x17 | 0x19 | 0x1C..=0x1F => self.actions.push(Action::Execute(b)),
            _ => self.sequence_byte(b),
        }
    }

    fn sequence_byte(&mut self, b: u8) {
        match self.state {
            State::Escape => {
                if let Some(s) = self.esc_string.take() {
                    if b == b'\\' {
                        self.state = s;
                        self.finish_string();
                        return;
                    }
                }
                match b {
                    b'[' => {
                        self.state = State::CsiEntry;
                    }
                    b']' => {
                        self.string.clear();
                        self.truncated = false;
                        self.state = State::OscString;
                    }
                    b'P' => {
                        self.string.clear();
                        self.truncated = false;
                        self.state = State::DcsEntry;
                    }
                    b'^' | b'_' | b'X' => {
                        self.string.clear();
                        self.truncated = false;
                        self.state = State::SosPmApcString;
                    }
                    _ if (0x20..=0x2F).contains(&b) => {
                        self.intermediates.push(b);
                        self.state = State::EscapeIntermediate;
                    }
                    _ if (0x30..=0x7E).contains(&b) => {
                        self.dispatch_escape(b);
                    }
                    _ => {
                        self.state = State::Ground;
                    }
                }
            }
            State::EscapeIntermediate => {
                if (0x20..=0x2F).contains(&b) {
                    self.intermediates.push(b);
                } else if (0x30..=0x7E).contains(&b) {
                    self.dispatch_escape(b);
                } else {
                    self.state = State::Ground;
                }
            }
            State::CsiEntry => {
                if (0x30..=0x39).contains(&b) || b == b';' || b == b':' {
                    self.param_byte(b);
                    self.state = State::CsiParam;
                } else if (0x3C..=0x3F).contains(&b) {
                    self.private = Some(b);
                } else if (0x20..=0x2F).contains(&b) {
                    self.intermediates.push(b);
                    self.state = State::CsiIntermediate;
                } else if (0x40..=0x7E).contains(&b) {
                    self.dispatch_csi(b);
                } else {
                    self.state = State::CsiIgnore;
                }
            }
            State::CsiParam => {
                if (0x30..=0x3F).contains(&b) {
                    self.param_byte(b);
                } else if (0x20..=0x2F).contains(&b) {
                    self.intermediates.push(b);
                    self.state = State::CsiIntermediate;
                } else if (0x40..=0x7E).contains(&b) {
                    self.dispatch_csi(b);
                } else {
                    self.state = State::CsiIgnore;
                }
            }
            State::CsiIntermediate => {
                if (0x20..=0x2F).contains(&b) {
                    self.intermediates.push(b);
                } else if (0x30..=0x3F).contains(&b) {
                    self.state = State::CsiIgnore;
                } else if (0x40..=0x7E).contains(&b) {
                    self.dispatch_csi(b);
                } else {
                    self.state = State::CsiIgnore;
                }
            }
            State::CsiIgnore => {
                if (0x40..=0x7E).contains(&b) {
                    self.actions.push(Action::Unsupported);
                    self.clear_sequence();
                    self.state = State::Ground;
                }
            }
            State::DcsEntry => {
                if (0x30..=0x3F).contains(&b) {
                    self.param_byte(b);
                    self.state = State::DcsParam;
                } else if (0x20..=0x2F).contains(&b) {
                    self.intermediates.push(b);
                    self.state = State::DcsIntermediate;
                } else if (0x40..=0x7E).contains(&b) {
                    self.dcs_final = Some(b);
                    self.string.clear();
                    self.truncated = false;
                    self.state = State::DcsPassthrough;
                } else {
                    self.state = State::DcsIgnore;
                }
            }
            State::DcsParam => {
                if (0x30..=0x3F).contains(&b) {
                    self.param_byte(b);
                } else if (0x20..=0x2F).contains(&b) {
                    self.intermediates.push(b);
                    self.state = State::DcsIntermediate;
                } else if (0x40..=0x7E).contains(&b) {
                    self.dcs_final = Some(b);
                    self.string.clear();
                    self.truncated = false;
                    self.state = State::DcsPassthrough;
                } else {
                    self.state = State::DcsIgnore;
                }
            }
            State::DcsIntermediate => {
                if (0x20..=0x2F).contains(&b) {
                    self.intermediates.push(b);
                } else if (0x40..=0x7E).contains(&b) {
                    self.dcs_final = Some(b);
                    self.string.clear();
                    self.truncated = false;
                    self.state = State::DcsPassthrough;
                } else {
                    self.state = State::DcsIgnore;
                }
            }
            State::DcsIgnore => {
                if (0x40..=0x7E).contains(&b) {
                    self.clear_sequence();
                    self.state = State::Ground;
                }
            }
            State::Ground => self.ground(b),
            State::OscString | State::DcsPassthrough | State::SosPmApcString => unreachable!(),
        }
    }

    fn param_byte(&mut self, b: u8) {
        if b == b';' || b == b':' {
            self.commit_param();
            if self.params.values.len() >= MAX_PARAMS {
                self.params.overflow = true;
            }
            return;
        }
        if !(0x30..=0x39).contains(&b) {
            return;
        }
        let cur = self.current_param.unwrap_or(0);
        self.current_param = Some(cur.saturating_mul(10).saturating_add(u16::from(b - b'0')));
    }

    fn commit_param(&mut self) {
        if self.current_param.is_none() && self.params.values.is_empty() {
            return;
        }
        if self.params.values.len() < MAX_PARAMS {
            self.params.values.push(self.current_param.unwrap_or(0));
        } else {
            self.params.overflow = true;
        }
        self.current_param = None;
    }

    fn clear_sequence(&mut self) {
        self.params = Params::new();
        self.current_param = None;
        self.intermediates.clear();
        self.private = None;
        self.string.clear();
        self.truncated = false;
        self.dcs_final = None;
        self.esc_string = None;
    }

    fn abort(&mut self) {
        self.clear_sequence();
        self.state = State::Ground;
    }

    fn string_byte(&mut self, b: u8) {
        if b == 0x07 {
            self.finish_string();
            return;
        }
        if self.string.len() >= MAX_OSC {
            self.truncated = true;
            return;
        }
        self.string.push(b);
    }

    fn finish_string(&mut self) {
        let payload = String::from_utf8_lossy(&self.string).into_owned();
        let truncated = self.truncated;
        match self.state {
            State::OscString => self.actions.push(Action::OscDispatch {
                truncated,
                payload,
            }),
            State::DcsPassthrough => {
                let final_byte = self.dcs_final.unwrap_or(0);
                self.commit_param();
                let params = self.params.values.clone();
                self.actions.push(Action::DcsDispatch {
                    final_byte,
                    params,
                    payload,
                });
            }
            _ => {}
        }
        self.clear_sequence();
        self.state = State::Ground;
    }

    fn dispatch_escape(&mut self, b: u8) {
        self.actions.push(Action::EscapeDispatch {
            intermediates: self.intermediates.clone(),
            final_byte: b,
        });
        self.clear_sequence();
        self.state = State::Ground;
    }

    fn dispatch_csi(&mut self, b: u8) {
        self.commit_param();
        self.actions.push(Action::CsiDispatch {
            private: self.private,
            params: self.params.clone(),
            intermediates: self.intermediates.clone(),
            final_byte: b,
        });
        self.clear_sequence();
        self.state = State::Ground;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &[u8]) -> Vec<Action> {
        Parser::new().advance(input)
    }

    #[test]
    fn plain_ascii_prints() {
        assert_eq!(
            parse(b"AB"),
            vec![Action::Print(u32::from('A')), Action::Print(u32::from('B'))]
        );
    }

    #[test]
    fn c0_controls_execute() {
        assert_eq!(parse(b"\r\n"), vec![Action::Execute(0x0D), Action::Execute(0x0A)]);
    }

    #[test]
    fn csi_with_parameters() {
        assert_eq!(
            parse(b"\x1b[1;31m"),
            vec![Action::CsiDispatch {
                private: None,
                params: Params { values: vec![1, 31], overflow: false },
                intermediates: vec![],
                final_byte: b'm',
            }]
        );
    }

    #[test]
    fn private_csi_marks_the_prefix() {
        let a = parse(b"\x1b[?25l");
        match &a[0] {
            Action::CsiDispatch { private, params, final_byte, .. } => {
                assert_eq!(*private, Some(b'?'));
                assert_eq!(params.get(0), 25);
                assert_eq!(*final_byte, b'l');
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn missing_parameters_are_zero_and_substitutable() {
        let a = parse(b"\x1b[m");
        match &a[0] {
            Action::CsiDispatch { params, .. } => {
                assert!(params.is_empty());
                assert_eq!(params.get(0), 0);
                assert_eq!(params.get_or(0, 1), 1);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn escape_dispatch_carries_intermediates() {
        assert_eq!(
            parse(b"\x1b(B"),
            vec![Action::EscapeDispatch {
                intermediates: vec![b'('],
                final_byte: b'B',
            }]
        );
    }

    #[test]
    fn osc_terminates_on_bel() {
        assert_eq!(
            parse(b"\x1b]0;title\x07"),
            vec![Action::OscDispatch { truncated: false, payload: "0;title".into() }]
        );
    }

    #[test]
    fn osc_terminates_on_string_terminator() {
        assert_eq!(
            parse(b"\x1b]0;title\x1b\\"),
            vec![Action::OscDispatch { truncated: false, payload: "0;title".into() }]
        );
    }

    #[test]
    fn can_aborts_any_sequence() {
        assert_eq!(
            parse(b"\x1b[1;31\x18m"),
            vec![Action::Print(u32::from('m'))],
            "CAN must abandon the CSI and return to ground"
        );
    }

    #[test]
    fn malformed_csi_resynchronises_instead_of_hanging() {
        let a = parse(b"\x1b[1;\x07" );
        assert!(!a.is_empty());
    }

    #[test]
    fn param_list_is_capped_and_flagged() {
        let mut input = b"\x1b[".to_vec();
        for _ in 0..80 {
            input.push(b'1');
            input.push(b';');
        }
        input.push(b'm');
        let a = parse(&input);
        match &a[0] {
            Action::CsiDispatch { params, .. } => {
                assert!(params.len() <= MAX_PARAMS, "params leaked past the cap");
                assert!(params.overflow);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn utf8_is_decoded_across_chunk_boundaries() {
        let bytes = "漢".as_bytes().to_vec();
        let mut p = Parser::new();
        let mut got = Vec::new();
        for b in bytes {
            got.extend(p.advance(&[b]));
        }
        assert_eq!(got, vec![Action::Print(u32::from('漢'))]);
    }

    #[test]
    fn invalid_utf8_yields_replacement_characters() {
        assert_eq!(parse(&[0xFF]), vec![Action::Print(0xFFFD)]);
        assert_eq!(parse(&[0xC0]), vec![Action::Print(0xFFFD)]);
        assert_eq!(
            parse(&[0xE6, 0x28]),
            vec![Action::Print(0xFFFD), Action::Print(u32::from('('))],
            "an invalid continuation must reprocess the byte as normal input"
        );
    }

    #[test]
    fn raw_c1_bytes_are_executed_not_decoded_as_utf8() {
        assert_eq!(
            parse(&[0x9D, b'a', 0x07]),
            vec![Action::OscDispatch { truncated: false, payload: "a".into() }],
            "0x9D is the 8-bit OSC introducer"
        );
    }

#[test]
    fn truncated_utf8_at_end_of_input_flushes_a_replacement() {
        let mut p = Parser::new();
        assert_eq!(p.advance(&[0xE6, 0xBC]), Vec::new(), "must not flush per chunk");
        assert_eq!(p.flush(), vec![Action::Print(0xFFFD)]);
    }

    #[test]
    fn parser_is_total_over_arbitrary_bytes() {
        let mut p = Parser::new();
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..200_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let byte = (s >> 24) as u8;
            let _ = p.advance(&[byte]);
        }
    }

    #[test]
    fn state_returns_to_ground_after_a_complete_sequence() {
        let mut p = Parser::new();
        let _ = p.advance(b"\x1b[1m");
        assert_eq!(p.state(), State::Ground);
        let _ = p.advance(b"\x1b]0;t\x07");
        assert_eq!(p.state(), State::Ground);
        let _ = p.advance(b"\x1b(B");
        assert_eq!(p.state(), State::Ground);
    }

    #[test]
    fn escape_restarts_a_sequence_mid_string() {
        let a = parse(b"\x1b]0;abc\x1b]1;def\x07");
        assert_eq!(a.len(), 1);
        match &a[0] {
            Action::OscDispatch { payload, .. } => assert_eq!(payload, "1;def"),
            other => panic!("{other:?}"),
        }
    }
}

