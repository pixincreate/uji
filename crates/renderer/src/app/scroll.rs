use std::cell::Cell;

#[derive(Debug, Clone, Copy)]
enum Anchor {
    Follow,
    At(usize),
}

#[derive(Debug)]
pub struct Scroll {
    anchor: Cell<Anchor>,
    resolved: Cell<usize>,
    viewport: Cell<usize>,
}

impl Default for Scroll {
    fn default() -> Self {
        Self {
            anchor: Cell::new(Anchor::Follow),
            resolved: Cell::new(0),
            viewport: Cell::new(0),
        }
    }
}

impl Scroll {
    pub fn resolve(&self, max: usize, viewport: usize) -> usize {
        let offset = match self.anchor.get() {
            Anchor::Follow => max,
            Anchor::At(at) => at.min(max),
        };
        self.anchor.set(if offset >= max {
            Anchor::Follow
        } else {
            Anchor::At(offset)
        });
        self.resolved.set(offset);
        self.viewport.set(viewport);
        offset
    }

    pub fn viewport(&self) -> usize {
        self.viewport.get()
    }

    pub fn follow(&self) {
        self.anchor.set(Anchor::Follow);
    }

    pub fn top(&self) {
        self.anchor.set(Anchor::At(0));
    }

    pub fn up(&self, lines: usize) {
        self.anchor
            .set(Anchor::At(self.resolved.get().saturating_sub(lines)));
    }

    pub fn down(&self, lines: usize) {
        self.anchor
            .set(Anchor::At(self.resolved.get().saturating_add(lines)));
    }
}
