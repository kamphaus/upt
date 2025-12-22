use chrono::{DateTime, Duration, Local, TimeDelta, Utc};
use clap::{Command, CommandFactory, Parser, Subcommand, crate_version};
use clap_complete::{Generator, Shell, generate};
use std::fs::File;
use std::io::{Write, stdout};
use std::ops::Sub;
use std::sync::mpsc::channel;
use std::time;
use std::{fs, io};

use chrono_humanize::{Accuracy, Tense};
use crossterm::{
    cursor::{Hide, Show},
    execute,
};
use simple_error::bail;

#[derive(Parser)]
#[clap(author = "Christophe Kamphaus", about = "A simple uptime CLI tool")]
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

    /// Show the system uptime, disregarding any resets
    #[arg(long)] // no short argument to prevent collision with start flag
    system: bool,

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
    Completion { shell: Shell },
}

/// Prints the given autocompletion for the passed shell and command to stdout
fn print_completions<G: Generator>(r#gen: G, cmd: &mut Command) {
    generate(r#gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

/// print the given start time, either in strict iso format or in simplified iso format
fn print_start(start: DateTime<Local>, strict_iso: bool) {
    if strict_iso {
        println!("Started {start:?}");
    } else {
        println!("Started {}", start.format("%Y-%m-%d %H:%M:%S"));
    }
}

fn render_duration(start: DateTime<Utc>, iso: bool) -> String {
    let duration = Utc::now().sub(start);
    if iso {
        return duration.to_string();
    }
    let truncated = TimeDelta::try_seconds(duration.num_seconds()).unwrap();
    let formatted =
        chrono_humanize::HumanTime::from(truncated).to_text_en(Accuracy::Precise, Tense::Present);
    formatted.replace(" and", ",")
}

/// Overwrite the previous line when printing the next one, passing the length of the line to be overwritten
fn clear_line(line_length: usize) {
    print!("\r");
    for _i in 0..line_length {
        // clear the line with spaces in case the next line is shorter
        print!(" ")
    }
    print!("\r")
}

use std::error::Error;
use std::path::PathBuf;

type BoxResult<T> = Result<T, Box<dyn Error>>;

fn get_system_start_time() -> Result<DateTime<Utc>, String> {
    let uptime = uptime_lib::get().unwrap();
    let now = Local::now();
    // We assume that the system uptime is not impacted by any timezone changes.
    // To ensure that any timezone changes do not impact the consistency between start datetime and duration
    // we always represent it in UTC and just print the start time in the local timezone.
    Ok(now
        .checked_sub_signed(Duration::from_std(uptime).unwrap())
        .unwrap()
        .with_timezone(&Utc))
}

fn get_start_time() -> Result<DateTime<Utc>, String> {
    let start_uptime = get_system_start_time()?;
    let persisted_uptime = read_time();
    if persisted_uptime.is_err() {
        //eprintln!("Could not get persisted uptime {}", persisted_uptime.err().unwrap());
        return Ok(start_uptime);
    }
    let parsed_datetime = persisted_uptime.unwrap();
    Ok(parsed_datetime.max(start_uptime))
}

fn get_file_path() -> BoxResult<PathBuf> {
    let mut p: PathBuf;
    match home::home_dir() {
        Some(path) => p = path,
        None => bail!("Impossible to get your home dir!"),
    }
    p.push(".upt");
    Ok(p)
}

fn persist_time(dt: DateTime<Utc>) -> BoxResult<()> {
    let path = get_file_path()?;
    let mut file = File::create(path.as_path())?;
    file.write_all(dt.to_rfc3339().as_bytes())?;
    Ok(())
}

fn read_time() -> BoxResult<DateTime<Utc>> {
    let persisted = fs::read_to_string(get_file_path()?)?;
    parse_time(persisted)
}

fn parse_time(date_str: String) -> BoxResult<DateTime<Utc>> {
    let parsed = DateTime::parse_from_rfc3339(date_str.trim())?;
    Ok(parsed.with_timezone(&Utc))
}

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::Completion { shell }) => {
            let mut app = Cli::command();
            print_completions(*shell, &mut app);
            return;
        }
        None => {
            // handle below
        }
    }
    if cli.version {
        let cli = Cli::command();
        println!(
            "{} {}",
            cli.get_display_name().unwrap_or_else(|| cli.get_name()),
            crate_version!()
        ); // same implementation as the default clap version command
        return;
    }
    let mut start = get_start_time().unwrap();
    let now = Local::now();
    if cli.reset {
        // reset the counter
        start = now.with_timezone(&Utc);
        let result = persist_time(start);
        if result.is_err() {
            eprintln!("Could not reset the uptime: {}", result.err().unwrap());
            return;
        }
    }
    if cli.system {
        let system_start = get_system_start_time();
        if system_start.is_err() {
            return;
        }
        start = system_start.unwrap()
    }
    if cli.start {
        print_start(start.with_timezone(&Local), cli.iso);
    }
    if cli.watch {
        let sleep_millis = time::Duration::from_millis(5);
        let (tx, rx) = channel();
        ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
            .expect("Error setting Ctrl-C handler");
        execute!(stdout(), Hide).expect("Could not hide terminal cursor.");
        loop {
            // print the current duration
            let duration = render_duration(start, cli.iso);
            print!("{duration}");

            // need to flush before waiting so we are sure that the up-to-date duration is displayed
            stdout().flush().expect("Could not flush stdout");

            if rx.recv_timeout(sleep_millis).is_ok() {
                // Received SIGTERM, perform cleanup by showing cursor again, go to a new line
                // so that the next prompt is displayed cleanly before terminating.
                execute!(stdout(), Show).expect("Could not hide terminal cursor.");
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDateTime, Utc};

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }

    #[test]
    fn test_date_parsing() {
        let nt = NaiveDateTime::from_timestamp_opt(1685871491, 0);
        let dt: DateTime<Utc> = DateTime::from_utc(nt.unwrap(), Utc);
        assert_eq!(
            parse_time("2023-06-04T09:38:11.000000000+00:00".to_string()).unwrap(),
            dt
        );
        assert_eq!(
            parse_time("2023-06-04T09:38:11.000000000+00:00\n".to_string()).unwrap(),
            dt
        );
        assert_eq!(
            parse_time("2030604T09:xy:11.000000000+00:00".to_string()).is_err(),
            true
        )
    }
}
