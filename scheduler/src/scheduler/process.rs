use super::{niceness::NicenessScheduler, tasks::Task};
use std::time::{Duration, Instant};

pub struct Process {
    pid: u32,
    name: String,
    task: Box<dyn Task>,
    niceness: i8,
    cpu_usage: Duration,
}

impl Process {
    const DEFAULT_NICENESS: i8 = 0;

    pub fn new(pid: u32, task: Box<dyn Task>) -> Self {
        Process::named(pid, "", task)
    }

    pub fn named(pid: u32, name: &str, task: Box<dyn Task>) -> Self {
        Process::with_niceness(pid, name, task, Process::DEFAULT_NICENESS)
    }

    pub fn with_niceness(pid: u32, name: &str, task: Box<dyn Task>, niceness: i8) -> Self {
        Self {
            pid,
            name: name.to_owned(),
            task,
            niceness,
            cpu_usage: Duration::ZERO,
        }
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn niceness(&self) -> i8 {
        self.niceness
    }

    pub fn badness(&self, cpu_elapsed: Duration) -> i64 {
        (self.cpu_usage.as_micros() as f64 / cpu_elapsed.as_micros() as f64
            * NicenessScheduler::CPU_USAGE_SCALE) as i64
            + (self.niceness * 2) as i64
    }

    pub fn cpu_usage_percentage(&self, cpu_elapsed: Duration) -> String {
        format!(
            "{}%",
            (self.cpu_usage.as_micros() as f64 / cpu_elapsed.as_micros() as f64 * 100.0).round()
        )
    }

    pub fn run(&mut self) -> String {
        let before_running = Instant::now();
        let output = self.task.run();
        self.cpu_usage += before_running.elapsed();
        output
    }
}
