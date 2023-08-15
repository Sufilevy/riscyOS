mod display;
mod niceness;
mod process;
mod round_robin;
mod runner;
mod tasks;

use std::time::Duration;

pub use niceness::NicenessScheduler;
pub use process::Process;
pub use round_robin::RoundRobinScheduler;
pub use runner::ProcessRunner;
pub use tasks::CounterTask;

const DEFAULT_TICK_RATE: Duration = Duration::from_millis(200);

pub trait Scheduler {
    const NAME: &'static str;

    fn processes(&self) -> &Vec<Process>;
    fn add_process(&mut self, process: Process);
    fn remove_process(&mut self, process_name: String) -> Option<Process>;
    fn schedule(&mut self) -> Option<&mut Process>;
    fn cpu_elapsed(&self) -> Duration;
    fn add_cpu_elapsed(&mut self, elapsed: Duration);
    fn current_process(&self) -> Option<&Process>;
    fn current_process_mut(&mut self) -> Option<&mut Process>;
}
