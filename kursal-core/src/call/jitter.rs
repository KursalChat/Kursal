use std::collections::BTreeMap;

pub enum JitterOut {
    Frame(Vec<u8>),
    Conceal,
    Empty,
}

pub struct JitterBuffer {
    target: usize,
    buf: BTreeMap<u64, Vec<u8>>,
    next: Option<u64>,
    primed: bool,
}

impl JitterBuffer {
    pub fn new(target_frames: usize) -> Self {
        Self {
            target: target_frames.max(1),
            buf: BTreeMap::new(),
            next: None,
            primed: false,
        }
    }

    pub fn push(&mut self, seq: u64, frame: Vec<u8>) {
        self.buf.insert(seq, frame);
    }

    pub fn pop(&mut self) -> JitterOut {
        if !self.primed {
            if self.buf.len() < self.target {
                return JitterOut::Empty;
            }
            self.primed = true;
            self.next = self.buf.keys().next().copied();
        }
        let next = match self.next {
            Some(n) => n,
            None => return JitterOut::Empty,
        };
        if let Some(frame) = self.buf.remove(&next) {
            self.next = Some(next.wrapping_add(1));
            return JitterOut::Frame(frame);
        }
        if self.buf.is_empty() {
            self.primed = false;
            self.next = None;
            return JitterOut::Empty;
        }
        self.next = Some(next.wrapping_add(1));
        JitterOut::Conceal
    }
}
