use clap::{Command, CommandFactory, Parser, Subcommand, crate_version};
use clap_complete::{Generator, Shell, generate};
use std::fs::File;
use std::io::{Write, stdout};
use std::sync::mpsc::channel;
use std::time;
use std::{fs, io};

use crossterm::{
    cursor::{Hide, Show},
    execute,
};
use jiff::tz::{Offset, TimeZone};
use jiff::{SignedDuration, Span, Timestamp, ZonedDateTime};
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
fn print_start(start: &ZonedDateTime, strict_iso: bool) {
    if strict_iso {
        println!(
            "Started {}",
            start.timestamp().display_with_offset(start.offset())
        );
    } else {
        println!("Started {}", start.strftime("%Y-%m-%d %H:%M:%S"));
    }
}

fn render_duration(start: Timestamp, iso: bool) -> String {
    let duration = Timestamp::now().duration_since(start);
    if iso {
        return format_iso_duration(duration);
    }
    humanize(duration.as_secs())
}

/// Format the duration in ISO 8601 format, expressed in (fractional) seconds, e.g. `PT3725.5S`
fn format_iso_duration(duration: SignedDuration) -> String {
    Span::new()
        .seconds(duration.as_secs())
        .nanoseconds(duration.subsec_nanos())
        .to_string()
}

/// Format the given number of seconds as human-readable text,
/// e.g. `1 week, 2 days, 3 hours, 4 minutes, 5 seconds`.
/// Months and years are approximated as 30 and 365 days respectively.
fn humanize(seconds: i64) -> String {
    const UNITS: [(i64, &str); 7] = [
        (365 * 86400, "year"),
        (30 * 86400, "month"),
        (7 * 86400, "week"),
        (86400, "day"),
        (3600, "hour"),
        (60, "minute"),
        (1, "second"),
    ];
    let mut remainder = seconds.abs();
    let mut parts = vec![];
    for (unit_seconds, name) in UNITS {
        let count = remainder / unit_seconds;
        remainder %= unit_seconds;
        if count == 1 {
            parts.push(format!("1 {name}"));
        } else if count > 1 {
            parts.push(format!("{count} {name}s"));
        }
    }
    if parts.is_empty() {
        return "0 seconds".to_string();
    }
    parts.join(", ")
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

fn get_system_start_time() -> Result<Timestamp, String> {
    let uptime = uptime_lib::get().map_err(|e| e.to_string())?;
    // We assume that the system uptime is not impacted by any timezone changes.
    // To ensure that any timezone changes do not impact the consistency between start datetime and duration
    // we always represent it as a timestamp (UTC) and just print the start time in the local timezone.
    Timestamp::now()
        .checked_sub(uptime)
        .map_err(|e| e.to_string())
}

fn get_start_time() -> Result<Timestamp, String> {
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

fn persist_time(dt: Timestamp) -> BoxResult<()> {
    let path = get_file_path()?;
    let mut file = File::create(path.as_path())?;
    // keep the explicit `+00:00` offset (RFC 3339) for compatibility with previous versions
    file.write_all(dt.display_with_offset(Offset::UTC).to_string().as_bytes())?;
    Ok(())
}

fn read_time() -> BoxResult<Timestamp> {
    let persisted = fs::read_to_string(get_file_path()?)?;
    parse_time(persisted)
}

fn parse_time(date_str: String) -> BoxResult<Timestamp> {
    Ok(date_str.trim().parse::<Timestamp>()?)
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
    if cli.reset {
        // reset the counter
        start = Timestamp::now();
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
        print_start(&start.to_zoned_datetime(TimeZone::system()), cli.iso);
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

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }

    #[test]
    fn test_date_parsing() {
        let dt = Timestamp::from_second(1685871491).unwrap();
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

    #[test]
    fn test_persisted_format() {
        let dt = Timestamp::from_second(1685871491).unwrap();
        assert_eq!(
            dt.display_with_offset(Offset::UTC).to_string(),
            "2023-06-04T09:38:11+00:00"
        );
    }

    #[test]
    fn test_humanize() {
        assert_eq!(humanize(0), "0 seconds");
        assert_eq!(humanize(1), "1 second");
        assert_eq!(humanize(61), "1 minute, 1 second");
        assert_eq!(humanize(3600), "1 hour");
        assert_eq!(
            humanize(9 * 86400 + 2 * 3600 + 3 * 60 + 4),
            "1 week, 2 days, 2 hours, 3 minutes, 4 seconds"
        );
        assert_eq!(
            humanize(2 * 365 * 86400 + 3 * 30 * 86400 + 5),
            "2 years, 3 months, 5 seconds"
        );
    }

    #[test]
    fn test_iso_duration() {
        assert_eq!(
            format_iso_duration(SignedDuration::new(3725, 500_000_000)),
            "PT3725.5S"
        );
        assert_eq!(
            format_iso_duration(SignedDuration::new(90061, 0)),
            "PT90061S"
        );
    }
}
