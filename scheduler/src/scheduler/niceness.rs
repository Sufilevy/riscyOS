use super::{Process, Scheduler, DEFAULT_TICK_RATE};
use std::time::{Duration, Instant};

pub struct NicenessScheduler {
    processes: Vec<Process>,
    current_process: usize,
    tick_rate: Duration,
    cpu_elapsed: Duration,
    last_tick: Instant,
}

impl NicenessScheduler {
    pub const CPU_USAGE_SCALE: f64 = 256.0;

    pub fn new() -> Self {
        NicenessScheduler::with_processes(Vec::new(), DEFAULT_TICK_RATE)
    }

    pub fn with_processes(processes: Vec<Process>, tick_rate: Duration) -> Self {
        Self {
            processes,
            current_process: 0,
            tick_rate,
            cpu_elapsed: Duration::ZERO,
            last_tick: Instant::now(),
        }
    }

    fn poll_process(&mut self) {
        if self.processes.len() < 2 {
            self.current_process = 0;
            return;
        }

        // Find the process with the least badnass, and make it the current process
        self.current_process = self
            .processes
            .iter()
            .enumerate()
            .map(|(index, process)| (index, process.badness(self.cpu_elapsed)))
            .min_by_key(|&(_, badnass)| badnass)
            .unwrap()
            .0;
    }
}

impl Scheduler for NicenessScheduler {
    const NAME: &'static str = "Niceness Scheduler";

    fn processes(&self) -> &Vec<Process> {
        &self.processes
    }

    fn add_process(&mut self, process: Process) {
        self.processes.push(process);
    }

    fn remove_process(&mut self, process_name: String) -> Option<Process> {
        match self
            .processes
            .iter()
            .position(|process| process.name() == process_name)
        {
            Some(index) => Some(self.processes.remove(index)),
            None => None,
        }
    }

    fn schedule(&mut self) -> Option<&mut Process> {
        if self.last_tick.elapsed() > self.tick_rate {
            self.last_tick = Instant::now();
            self.poll_process();
        }
        self.current_process_mut()
    }

    fn current_process(&self) -> Option<&Process> {
        self.processes.get(self.current_process)
    }

    fn current_process_mut(&mut self) -> Option<&mut Process> {
        self.processes.get_mut(self.current_process)
    }

    fn cpu_elapsed(&self) -> Duration {
        self.cpu_elapsed
    }

    fn add_cpu_elapsed(&mut self, elapsed: Duration) {
        self.cpu_elapsed += elapsed;
    }
}
