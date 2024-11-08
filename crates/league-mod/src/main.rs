use clap::{Parser, Subcommand};
use commands::{init_mod_project, InitModProjectArgs};

mod commands;
mod utils;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Init {
        #[arg(short, long)]
        name: String,
        #[arg(short, long)]
        display_name: Option<String>,
        #[arg(short, long)]
        output_dir: Option<String>,
    },
    Pack {
        #[arg(short, long, default_value = "artifacts")]
        output: String,
    },
}

fn main() -> eyre::Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Init {
            name,
            display_name,
            output_dir,
        } => init_mod_project(InitModProjectArgs {
            name,
            display_name,
            output_dir,
        }),
        Commands::Pack { output } => {
            println!("Packing mod to directory: {}", output);
            // Add packing logic here
            Ok(())
        }
    }
}
