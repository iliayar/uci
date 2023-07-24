use std::collections::VecDeque;

pub struct LinesQueue {
    data: Vec<u8>,
    lines: VecDeque<String>,
    has_changed: bool,
}

impl LinesQueue {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            lines: VecDeque::new(),
            has_changed: false,
        }
    }

    pub fn next(&mut self) -> Option<String> {
        if self.has_changed {
            self.parse_lines();
            self.has_changed = false;
        }

        self.lines.pop_front()
    }

    pub fn push(&mut self, mut data: Vec<u8>) {
        self.data.append(&mut data);
        self.has_changed = true;
    }

    pub fn finish(mut self) -> Vec<String> {
        self.parse_lines();

        if !self.data.is_empty() {
            self.lines
                .push_back(String::from_utf8_lossy(&self.data).to_string());
        }

        self.lines.into_iter().collect()
    }

    fn parse_lines(&mut self) {
        let mut begin = 0usize;
        for (i, c) in self.data.iter().enumerate() {
            if c == &b'\n' {
                self.lines
                    .push_back(String::from_utf8_lossy(&self.data[begin..i]).to_string());
                begin = i + 1;
            }
        }

        self.data = self.data.split_off(begin);
    }
}
