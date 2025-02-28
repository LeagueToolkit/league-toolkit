use clap::{Parser, Subcommand};
use commands::{init_mod_project, pack_mod_project, InitModProjectArgs, PackModProjectArgs};

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
        name: Option<String>,
        #[arg(short, long)]
        display_name: Option<String>,
        #[arg(short, long)]
        output_dir: Option<String>,
    },
    Pack {
        /// The path to the mod config file
        #[arg(short, long)]
        config_path: Option<String>,

        /// The resulting file name of the mod package
        #[arg(short, long)]
        file_name: Option<String>,

        /// The directory to output the mod package to
        #[arg(short, long, default_value = "/build")]
        output_dir: String,
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
        Commands::Pack {
            config_path,
            file_name,
            output_dir,
        } => pack_mod_project(PackModProjectArgs {
            config_path,
            file_name,
            output_dir,
        }),
    }
}
