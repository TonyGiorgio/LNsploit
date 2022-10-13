use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Toast {
    pub message: Arc<String>,
    ticks_left: usize,
    pub good_news: bool,
}

impl Toast {
    pub fn new(message: &str, good_news: bool) -> Self {
        Self {
            message: Arc::new(message.into()),
            ticks_left: 20,
            good_news,
        }
    }

    pub fn tick_tock(&mut self) {
        if (self.ticks_left > 0) {
            self.ticks_left = self.ticks_left - 1;
        }
    }

    pub fn should_disappear(&self) -> bool {
        self.ticks_left == 0
    }
}
