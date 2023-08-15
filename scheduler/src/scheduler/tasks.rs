use std::time::Duration;

pub trait Task {
    fn run(&mut self) -> String;
}

pub struct CounterTask {
    counter: u32,
}

impl CounterTask {
    pub fn new() -> Self {
        Self { counter: 0 }
    }
}

impl Task for CounterTask {
    fn run(&mut self) -> String {
        self.counter += 1;
        std::thread::sleep(Duration::from_millis(1));
        self.counter.to_string()
    }
}
