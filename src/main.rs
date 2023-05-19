use clap::{CommandFactory, Parser, Subcommand, Command, crate_version};
use clap_complete::{generate, Generator, Shell};
use std::io;
use std::time;
use std::io::{stdout, Write};
use std::ops::Sub;
use std::sync::mpsc::channel;
use chrono::{DateTime, Duration, Local, Utc};
use chrono;
use chrono_humanize::{Accuracy, Tense};
use crossterm::{
    execute,
    cursor::{Hide, Show},
};

#[derive(Parser)]
#[clap(author="Christophe Kamphaus", about="A simple uptime CLI tool")]
#[command(author, about, long_about = None)]
struct Cli {
    /// Reset the uptime to now
    #[arg(short, long)]
    reset: bool,

    /// Watch the uptime
    #[arg(short, long)]
    watch: bool,

    /// Print start date time
    #[arg(short, long)]
    start: bool,

    /// Instead of human readable duration print in ISO 8601 format
    #[arg(short, long)]
    iso: bool,

    /// Print version
    #[arg(short, long)]
    version: bool, // we explicitly use version flag, instead of the clap version macro so we can have lowercase 'v' short flag

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate autocompletion scripts for upt for the specified shell.
    Completion {
        shell: Shell
    },
}

/// Prints the given autocompletion for the passed shell and command to stdout
fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

/// print the given start time, either in strict iso format or in simplified iso format
fn print_start(start: DateTime<Local>, strict_iso: bool) {
    if strict_iso {
        println!("Started {:?}", start);
    } else {
        println!("Started {}", start.format("%Y-%m-%d %H:%M:%S").to_string());
    }
}

fn render_duration(start: DateTime<Utc>, iso: bool) -> String {
    let duration = Utc::now().sub(start);
    if iso {
        return duration.to_string()
    }
    let truncated = Duration::seconds(duration.num_seconds());
    let formatted = chrono_humanize::HumanTime::from(truncated).to_text_en(Accuracy::Precise, Tense::Present);
    return formatted.replace(" and", ",");
}

/// Overwrite the previous line when printing the next one, passing the length of the line to be overwritten
fn clear_line(line_length: usize) {
    for _i in 0..line_length {
        print!("{}", '\r')
    }
    for _i in 0..line_length { // clear the line with spaces in case the next line is shorter
        print!("{}", ' ')
    }
    for _i in 0..line_length {
        print!("{}", '\r')
    }
}

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::Completion { shell}) => {
            let mut app = Cli::command();
            print_completions(*shell, &mut app);
            return
        }
        None => {
            // handle below
        }
    }
    if cli.version {
        let cli = Cli::command();
        println!("{} {}", cli.get_display_name().unwrap_or_else(|| cli.get_name()), crate_version!()); // same implementation as the default clap version command
        return;
    }
    let start_result = uptime_lib::get();
    if start_result.is_err() {
        eprintln!("Cannot get uptime of system {}", start_result.unwrap_err());
        return;
    }
    let now = Local::now();
    // We assume that the system uptime is not impacted by any timezone changes.
    // To ensure that any timezone changes do not impact the consistency between start datetime and duration
    // we always represent it in UTC and just print the start time in the local timezone.
    let mut start = now.checked_sub_signed(Duration::from_std(start_result.unwrap()).unwrap()).unwrap().with_timezone(&Utc);
    // TODO: implement persistence
    if cli.reset {
        // reset the counter
        start = now.with_timezone(&Utc);  // TODO: implement persistence
    }
    if cli.start {
        print_start(start.with_timezone(&Local), cli.iso);
    }
    if cli.watch {
        let sleep_millis = time::Duration::from_millis(5);
        let (tx, rx) = channel();
        ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
            .expect("Error setting Ctrl-C handler");
        execute!(
            stdout(),
            Hide
        ).expect("Could not hide terminal cursor.");
        loop {
            // print the current duration
            let duration = render_duration(start, cli.iso);
            print!("{}", duration);

            // need to flush before waiting so we are sure that the up-to-date duration is displayed
            stdout().flush().expect("Could not flush stdout");

            if rx.recv_timeout(sleep_millis).is_ok() {
                // Received SIGTERM, perform cleanup by showing cursor again, go to a new line
                // so that the next prompt is displayed cleanly before terminating.
                execute!(
                    stdout(),
                    Show
                ).expect("Could not hide terminal cursor.");
                println!();
                break;
            }

            // Clear line after wait to minimize the time an empty line is displayed.
            clear_line(duration.len())
        }
    } else {
        println!("{}", render_duration(start, cli.iso))
    }
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}
