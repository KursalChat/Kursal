use std::collections::BTreeMap;

const MAX_TARGET_FRAMES: usize = 10;
const MAX_BUFFERED_FRAMES: usize = 25;
const CLEAN_POPS_TO_SHRINK: u32 = 250;

pub enum JitterOut {
    Frame(Vec<u8>),
    Conceal,
    Empty,
}

pub struct JitterBuffer {
    min_target: usize,
    target: usize,
    buf: BTreeMap<u64, Vec<u8>>,
    next: Option<u64>,
    primed: bool,
    held: bool,
    clean: u32,
}

impl JitterBuffer {
    pub fn new(target_frames: usize) -> Self {
        let target = target_frames.max(1);
        Self {
            min_target: target,
            target,
            buf: BTreeMap::new(),
            next: None,
            primed: false,
            held: false,
            clean: 0,
        }
    }

    pub fn target(&self) -> usize {
        self.target
    }

    pub fn push(&mut self, seq: u64, frame: Vec<u8>) {
        if self.primed && self.next.is_some_and(|next| seq < next) {
            self.widen();
            return;
        }
        self.buf.insert(seq, frame);
        while self.buf.len() > MAX_BUFFERED_FRAMES {
            let Some(&oldest) = self.buf.keys().next() else {
                break;
            };
            self.buf.remove(&oldest);
            if self.next.is_some_and(|next| next <= oldest) {
                self.next = self.buf.keys().next().copied();
            }
        }
    }

    pub fn pop(&mut self) -> JitterOut {
        if !self.primed {
            if self.buf.len() < self.target {
                return JitterOut::Empty;
            }
            self.primed = true;
            self.held = false;
            self.next = self.buf.keys().next().copied();
        }
        let Some(next) = self.next else {
            return JitterOut::Empty;
        };
        if let Some(frame) = self.buf.remove(&next) {
            self.next = Some(next.wrapping_add(1));
            self.held = false;
            self.tighten();
            return JitterOut::Frame(frame);
        }
        if self.buf.is_empty() {
            self.primed = false;
            self.next = None;
            self.held = false;
            return JitterOut::Empty;
        }
        if !self.held {
            self.held = true;
            return JitterOut::Conceal;
        }
        self.held = false;
        self.widen();
        let Some(&resume) = self.buf.keys().next() else {
            return JitterOut::Conceal;
        };
        let frame = self.buf.remove(&resume).unwrap_or_default();
        self.next = Some(resume.wrapping_add(1));
        JitterOut::Frame(frame)
    }

    fn widen(&mut self) {
        self.clean = 0;
        if self.target < MAX_TARGET_FRAMES {
            self.target += 1;
        }
    }

    fn tighten(&mut self) {
        if self.target <= self.min_target {
            return;
        }
        self.clean += 1;
        if self.clean >= CLEAN_POPS_TO_SHRINK {
            self.clean = 0;
            self.target -= 1;
        }
    }
}
