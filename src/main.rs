use chrono::{DateTime, Duration, Local};
use crossbeam_channel::{Receiver, Sender};
use indicatif::{ProgressBar, ProgressStyle};
use ion_shell::Shell;
use log::{debug, error, info, trace, warn};
use simplelog::*;
use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    path::PathBuf,
    sync::Arc,
    thread,
};
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

    /// Print the jobs to stdout, but don't execute them
    #[structopt(long = "dry-run")]
    dry_run: bool,

    /// Halt on error in a command
    #[structopt(long = "halt-on-error")]
    halt: bool,

    /// Ask the user before running each command
    #[structopt(short, long)]
    interactive: bool,

    /// Start n jobs in parallel. Defaults to the number of cores available. 0 indicates to run one
    /// thread per job
    #[structopt(short, long)]
    jobs: Option<usize>,

    /// Each line of the argfile will be treated as a replacement on the input
    #[structopt(short, long = "arg-file", parse(from_os_str))]
    argfile: Option<PathBuf>,

    // Positionals
    /// The command to run. '{}' tokens will be replaced with the list of arguments
    command: String,
    /// The list of arguments
    arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct JobResult {
    seq: usize,
    exit_code: i32,
    start: DateTime<Local>,
    duration: Duration,
    cmd: String,
}

fn add_jobs(
    command: Arc<String>,
    arguments: Vec<String>,
    argfile: Option<PathBuf>,
    ask: bool,
    tx: Sender<String>,
) {
    let mut i = 0;
    let mut always = false;
    let mut start = |arg: String| {
        let command = command.replace("{}", &arg);
        if ask && !always {
            loop {
                eprint!("Do '{}'? [Y/n/a]: ", command);
                let mut input = String::new();
                if io::stdin()
                    .read_line(&mut input)
                    .expect("Failed to read line")
                    == 0
                {
                    error!("Could not read from stdin in interactive mode");
                    std::process::exit(1);
                }
                match input.trim() {
                    "y" | "Y" | "yes" | "Yes" | "" => break,
                    "n" | "N" | "no" | "No" => return,
                    "a" | "A" | "all" | "All" | "always" | "Always" => {
                        always = true;
                        break;
                    }
                    _ => eprintln!("Invalid choice"),
                }
            }
        }
        debug!("Starting {}: '{}'", i, command.replace("{}", &arg));
        tx.send(arg.to_string()).unwrap();
        i += 1;
    };
    if arguments.is_empty() {
        if let Some(argfile) = argfile {
            let file = match File::open(&argfile) {
                Err(err) => {
                    error!(
                        "Could not open arg file '{}' for reading: {}",
                        argfile.to_string_lossy(),
                        err
                    );
                    std::process::exit(1);
                }
                Ok(file) => file,
            };
            for arg in BufReader::new(file).lines() {
                let arg = arg.expect("Could not read the file");
                start(arg)
            }
        } else {
            for arg in BufReader::new(io::stdin()).lines() {
                let arg = arg.expect("Could not stdin");
                start(arg)
            }
        }
    } else {
        arguments.into_iter().for_each(start);
    }
}

fn create_logger(opts: &Opts) {
    let level = match (opts.quiet, opts.verbose) {
        (true, _) => LevelFilter::Error,
        (_, 0) => LevelFilter::Warn,
        (_, 1) => LevelFilter::Info,
        (_, 2) => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    let config = Config::default();
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

// TODO: Add a feature to use Ion as an external command
fn run(check_only: bool, cmd: &str) -> i32 {
    let mut shell = Shell::default();
    shell.opts_mut().no_exec = check_only;
    match shell.execute_command(cmd.as_bytes()) {
        Err(err) => {
            error!("could not execute command '{}': {}", cmd, err);
            1
        }
        Ok(_) => shell.previous_status().as_os_code(),
    }
}

fn start_workers(
    n: usize,
    check_only: bool,
    task: &Arc<String>,
    jobs: Receiver<String>,
    results: Sender<JobResult>,
) {
    debug!("Starting {} worker threads", n);
    for seq in 0..n {
        let jobs = jobs.clone();
        let results = results.clone();
        let task = task.clone();
        thread::spawn(move || {
            while let Ok(job) = jobs.recv() {
                let start = Local::now();
                let cmd = task.replace("{}", &job);
                let exit_code = run(check_only, &cmd);
                let duration = start.signed_duration_since(Local::now());
                results
                    .send(JobResult {
                        seq,
                        start,
                        duration,
                        cmd,
                        exit_code,
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
            .unwrap_or_else(num_cpus::get)
            .min(opts.arguments.len()),
        opts.dry_run,
        &command,
        rx,
        rtx,
    );

    let pb = if opts.arguments.is_empty() {
        ProgressBar::new_spinner()
    } else {
        ProgressBar::new(opts.arguments.len() as u64)
    };
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:.green}: [{elapsed_precise}] [{bar:40}] {pos:>7}/{len:7} ({eta})")
            .progress_chars("????????????????????????  "),
    );
    pb.set_prefix("Progress");
    add_jobs(command, opts.arguments, opts.argfile, opts.interactive, tx);

    let mut exit = 0;
    while let Ok(result) = rrx.recv() {
        pb.inc(1);
        if !opts.dry_run {
            info!("'{}' took {}s", result.cmd, result.duration);
            if result.exit_code != 0 {
                warn!(
                    "'{}' exited with status code {}",
                    result.cmd, result.exit_code
                );
                if opts.halt {
                    std::process::exit(1);
                } else {
                    exit = 1;
                }
            }
        }
    }
    pb.finish_with_message("done");
    std::process::exit(exit);
}
