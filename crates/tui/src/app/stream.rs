const EASE: usize = 6;
const MIN: usize = 3;
const MAX: usize = 120;
const BURST: usize = 4096;

#[derive(Debug, Default)]
pub struct Stream {
    text: String,
    revealed: usize,
}

impl Stream {
    pub fn push(&mut self, delta: &str) {
        self.text.push_str(delta);
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.revealed = 0;
    }

    pub fn visible(&self) -> &str {
        &self.text[..self.revealed.min(self.text.len())]
    }

    pub fn revealing(&self) -> bool {
        self.revealed < self.text.len()
    }

    pub fn reveal_all(&mut self) {
        self.revealed = self.text.len();
    }

    pub fn reveal_step(&mut self) -> bool {
        let backlog = self.text.len().saturating_sub(self.revealed);
        if backlog == 0 {
            return false;
        }
        let step = if backlog > BURST {
            backlog
        } else {
            backlog.div_ceil(EASE).clamp(MIN, MAX)
        };
        let mut at = self.revealed.saturating_add(step).min(self.text.len());
        while !self.text.is_char_boundary(at) {
            at = at.saturating_add(1);
        }
        self.revealed = at;
        true
    }
}
