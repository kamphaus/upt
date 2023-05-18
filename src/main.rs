use clap::{CommandFactory, Parser, Subcommand, Command};
use clap_complete::{generate, Generator, Shell};
use std::io;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Optional name to operate on
    name: Option<String>,

    /// Turn debugging information on
    //#[arg(short, long, action = clap::ArgAction::Count)]
    //debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// does testing things
    Completion {
        shell: Option<Shell>
    },
}

fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

fn main() {
    let cli = Cli::parse();
    let mut app = Cli::command();
    match &cli.command {
        Some(Commands::Completion { shell}) => {
            let s = shell.unwrap_or(Shell::Bash);
            print_completions(s, &mut app);
            println!("Not printing testing lists...");
        }
        None => {
            println!("Hello, world!");
        }
    }
}
