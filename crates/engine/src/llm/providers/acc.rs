use crate::session::model::ToolCall;

pub struct Partial {
    index: usize,
    pub id: String,
    pub name: String,
    pub arguments: String,
}

impl Partial {
    fn new(index: usize) -> Self {
        Self {
            index,
            id: String::new(),
            name: String::new(),
            arguments: String::new(),
        }
    }
}

#[derive(Default)]
pub struct ToolAcc {
    calls: Vec<Partial>,
}

impl ToolAcc {
    pub fn entry(&mut self, index: usize) -> &mut Partial {
        let position = self
            .calls
            .iter()
            .position(|call| call.index == index)
            .unwrap_or(self.calls.len());
        if position == self.calls.len() {
            self.calls.push(Partial::new(index));
        }
        &mut self.calls[position]
    }

    pub fn finish(self) -> Vec<ToolCall> {
        let mut calls = self.calls;
        calls.sort_by_key(|call| call.index);
        calls
            .into_iter()
            .map(|call| ToolCall {
                id: call.id,
                name: call.name,
                arguments: call.arguments,
            })
            .collect()
    }
}
