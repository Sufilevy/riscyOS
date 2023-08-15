mod scheduler;

use crossterm::{
    execute,
    terminal::{Clear, ClearType},
};
use scheduler::{CounterTask, NicenessScheduler, Process, ProcessRunner};
use std::{io, time::Duration};

fn main() -> Result<(), io::Error> {
    execute!(io::stdout(), Clear(ClearType::All))?;

    let scheduler = NicenessScheduler::with_processes(
        vec![
            Process::with_niceness(0, "Process 0", Box::new(CounterTask::new()), 10),
            Process::with_niceness(1, "Process 1", Box::new(CounterTask::new()), 20),
            Process::with_niceness(3, "Process 2", Box::new(CounterTask::new()), -19),
            Process::named(10, "Process 3", Box::new(CounterTask::new())),
            Process::with_niceness(12, "Process 4", Box::new(CounterTask::new()), -2),
        ],
        Duration::from_millis(500),
    );
    let mut runner = ProcessRunner::new(scheduler);

    while runner.run() {}

    execute!(io::stdout(), Clear(ClearType::All))?;
    Ok(())
}
