use std::time::Instant;

use super::{display::DisplayTerminal, Scheduler};

pub enum RunnerEvent {
    Quit,
    Pause,
    Resume,
    Step,
    None,
}

pub struct ProcessRunner<S> {
    terminal: DisplayTerminal,
    scheduler: S,
    paused: bool,
}

impl<S: Scheduler> ProcessRunner<S> {
    pub fn new(scheduler: S) -> Self {
        let terminal = DisplayTerminal::new().expect("Failed to create a terminal.");

        Self {
            terminal,
            scheduler,
            paused: false,
        }
    }

    fn run_process(&mut self) -> String {
        if let Some(process) = self.scheduler.schedule() {
            let start_time = Instant::now();
            let output = process.run();
            self.scheduler.add_cpu_elapsed(start_time.elapsed());
            output
        } else {
            String::new()
        }
    }

    // Returns false if the program should quit
    pub fn run(&mut self) -> bool {
        let process_output = if !self.paused {
            self.run_process()
        } else {
            String::new()
        };
        self.terminal.draw(&self.scheduler, process_output);

        match self.terminal.get_input() {
            RunnerEvent::Quit => return false,
            RunnerEvent::Pause if !self.paused => self.paused = true,
            RunnerEvent::Resume if self.paused => self.paused = false,
            RunnerEvent::Step if self.paused => {
                self.run_process();
            }
            _ => {}
        }
        true
    }
}
