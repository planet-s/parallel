use log::{debug, error, info, trace, warn};
use simplelog::*;
use std::fs::File;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "parallelion", about = "An example of StructOpt usage.")]
struct Opt {
    /// Use NUL as delimiter instead of \n (newline). Useful if arguments can contain \n
    #[structopt(short = "0", long)]
    null: bool,

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
    /// Timestamp (sec, ms, ns, none)
    #[structopt(long)]
    timestamp: Option<String>,

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
    jobs: Option<u8>,

    /// Each line of the argfile will be treated as a replacement on the input
    #[structopt(short, long = "arg-file", parse(from_os_str))]
    argfiles: Vec<PathBuf>,

    // Positionals
    /// The command to run. '{}' tokens will be replaced with the list of arguments
    command: String,
    /// The list of arguments
    arguments: Vec<String>,
}

fn main() {
    let opt = Opt::from_args();
    println!("{:#?}", opt);

    let level = match (opt.quiet, opt.verbose) {
        (true, _) => LevelFilter::Error,
        (_, 0) => LevelFilter::Warn,
        (_, 1) => LevelFilter::Info,
        (_, 2) => LevelFilter::Debug,
        (..) => LevelFilter::Trace,
    };
    let mut config = Config::default();
    // config.time_format = opt.timestamp;
    let mut loggers: Vec<Box<dyn SharedLogger>> =
        vec![TermLogger::new(level, config, TerminalMode::Stderr).unwrap()];
    if let Some(file) = opt.log {
        loggers.push(WriteLogger::new(
            LevelFilter::Info,
            config,
            File::create(file).unwrap(),
        ));
    }
    CombinedLogger::init(loggers).unwrap();

    trace!("trace message");
    debug!("debug message");
    info!("info message");
    warn!("warn message");
    error!("error message");
}
