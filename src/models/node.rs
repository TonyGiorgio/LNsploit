#[derive(Clone)]
pub struct Node {
    pub name: String,
}

impl Node {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}
