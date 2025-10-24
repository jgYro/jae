use std::collections::VecDeque;

pub struct KillRing {
    ring: VecDeque<String>,
    max_size: usize,
}

impl KillRing {
    pub fn new() -> Self {
        Self {
            ring: VecDeque::new(),
            max_size: 60,
        }
    }

    pub fn push(&mut self, text: String) {
        if text.is_empty() {
            return;
        }
        self.ring.push_front(text);
        self.truncate();
    }

    pub fn append_to_last(&mut self, text: String) {
        if let Some(last) = self.ring.pop_front() {
            self.push(last + &text);
        } else {
            self.push(text);
        }
    }

    pub fn yank(&self) -> Option<&String> {
        self.ring.front()
    }

    fn truncate(&mut self) {
        while self.ring.len() > self.max_size {
            self.ring.pop_back();
        }
    }
}