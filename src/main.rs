use crossbeam_channel::{Receiver, Sender};
use log::{debug, error, info, trace, warn};
use simplelog::*;
use std::fmt;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "parallelion", about = "An example of StructOpt usage.")]
struct Opts {
    /// Show progress
    ///
    /// Displays % of jobs completed, ETA, number of jobs running, number of jobs started
    #[structopt(short, long)]
    progress: bool,

    /// Silence all output
    #[structopt(short = "q", long = "quiet")]
    quiet: bool,
    /// Increase verbosity (0 = normal, 1 = info, 2 = debug)
    #[structopt(short, long, parse(from_occurrences))]
    verbose: usize,
    /// Log the executed jobs to the following file
    ///
    /// The format used is a json with the following fields: sequence number (seq), start time
    /// (start), duration in floating-point seconds (duration), command run (cmd), exit status (exit_code)
    #[structopt(short, long, parse(from_os_str))]
    log: Option<PathBuf>,
    // /// Timestamp (sec, ms, ns, none)
    // #[structopt(long)]
    // timestamp: Option<String>,

    // dry run
    /// Print the jobs to stdout, but don't execute them
    #[structopt(long = "dry-run")]
    dry_run: bool,

    /// Halt on error in a command
    #[structopt(long = "halt-on-error")]
    halt: bool,

    /// Ask the user before running each command
    #[structopt(long)]
    interactive: bool,

    /// Start n jobs in parallel. Defaults to the number of cores available. 0 indicates to run one
    /// thread per job
    #[structopt(short, long)]
    jobs: Option<usize>,

    /// Each line of the argfile will be treated as a replacement on the input
    #[structopt(short, long = "arg-file", parse(from_os_str))]
    argfiles: Vec<PathBuf>,

    // Positionals
    /// The command to run. '{}' tokens will be replaced with the list of arguments
    command: String,
    /// The list of arguments
    arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct JobResult {
    seq: usize,
    exit_code: usize,
    start: u8,
    duration: f64,
    cmd: String,
}

impl fmt::Display for JobResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{}] '{}', started at {}, took {}s and exited with code {}",
            self.seq, self.cmd, self.start, self.duration, self.exit_code
        )
    }
}

fn create_logger(opts: &Opts) {
    let level = match (opts.quiet, opts.verbose) {
        (true, _) => LevelFilter::Error,
        (_, 0) => LevelFilter::Warn,
        (_, 1) => LevelFilter::Info,
        (_, 2) => LevelFilter::Debug,
        (..) => LevelFilter::Trace,
    };
    let config = Config::default();
    // config.time_format = opt.timestamp;
    let mut loggers: Vec<Box<dyn SharedLogger>> =
        vec![TermLogger::new(level, config, TerminalMode::Stderr).unwrap()];
    if let Some(file) = &opts.log {
        loggers.push(WriteLogger::new(
            LevelFilter::Info,
            config,
            File::create(file).unwrap(),
        ));
    }
    CombinedLogger::init(loggers).unwrap();
}

fn start_workers(n: usize, task: &Arc<String>, jobs: Receiver<String>, results: Sender<JobResult>) {
    debug!("Starting {} worker threads", n);
    for seq in 0..n {
        let jobs = jobs.clone();
        let results = results.clone();
        let task = task.clone();
        thread::spawn(move || {
            while let Ok(job) = jobs.recv() {
                let cmd = task.replace("{}", &job);
                results
                    .send(JobResult {
                        seq,
                        start: 0,
                        duration: 0.,
                        cmd,
                        exit_code: 0,
                    })
                    .unwrap();
            }
        });
    }
}

fn main() {
    let opts = Opts::from_args();
    trace!("{:#?}", opts);
    create_logger(&opts);

    let (tx, rx) = crossbeam_channel::unbounded();
    let (rtx, rrx) = crossbeam_channel::unbounded();

    let command = Arc::new(opts.command);
    start_workers(
        opts.jobs
            .unwrap_or(num_cpus::get())
            .min(opts.arguments.len()),
        &command,
        rx,
        rtx,
    );

    for (i, argument) in opts.arguments.into_iter().enumerate() {
        debug!("Starting {}: '{}'", i, command.replace("{}", &argument));
        tx.send(argument).unwrap();
    }
    std::mem::drop(tx);

    while let Ok(result) = rrx.recv() {
        if result.exit_code == 0 {
            info!("{}", result);
        } else {
            warn!("{}", result);
        }
    }
}
