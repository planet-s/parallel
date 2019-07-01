use crossbeam_channel::{Receiver, Sender};
use log::{debug, error, info, trace, warn};
use simplelog::*;
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
    /// (start), duration in floating-point seconds (duration), command run (cmd), exit status (status)
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

fn start_workers(n: usize, task: Arc<String>, jobs: Receiver<String>, results: Sender<usize>) {
    info!("Starting {} worker threads", n);
    for _ in 0..n {
        let jobs = jobs.clone();
        let results = results.clone();
        let task = task.clone();
        thread::spawn(move || {
            while let Ok(job) = jobs.recv() {
                let job = task.replace("{}", &job);
                info!("Command: {}", job);
                results.send(1).unwrap();
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
        command,
        rx,
        rtx,
    );

    for argument in opts.arguments {
        tx.send(argument).unwrap();
    }
    std::mem::drop(tx);

    while let Ok(result) = rrx.recv() {
        eprintln!("result: {}", result);
    }
}
