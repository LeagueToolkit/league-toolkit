mod commands;
mod fantome;
mod utils;

use clap::{Parser, Subcommand};
use commands::{
    extract_mod_package, fantome_to_project, info_mod_package, init_mod_project, pack_mod_project,
    ExtractModPackageArgs, FantomeToProjectArgs, InfoModPackageArgs, InitModProjectArgs,
    PackModProjectArgs,
};

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
        #[arg(short, long, default_value = "build")]
        output_dir: String,
    },
    /// Show information about a mod package
    Info {
        /// The path to the mod package file
        #[arg(short, long)]
        file_path: String,
    },
    /// Extract a mod package to a directory
    Extract {
        /// The path to the mod package file
        #[arg(short, long)]
        file_path: String,

        /// The directory to extract the mod package to
        #[arg(short, long, default_value = "extracted")]
        output_dir: String,
    },
    FantomeToProject {
        /// The path to the fantome file
        #[arg(short, long)]
        fantome_path: String,

        /// Where to create the mod project
        #[arg(short, long)]
        output_dir: String,

        /// The path to the hashtable file to use for WAD files
        #[arg(short, long)]
        hashtable_path: String,
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
        Commands::Info { file_path } => info_mod_package(InfoModPackageArgs { file_path }),
        Commands::Extract {
            file_path,
            output_dir,
        } => extract_mod_package(ExtractModPackageArgs {
            file_path,
            output_dir,
        }),
        Commands::FantomeToProject {
            fantome_path,
            output_dir,
            hashtable_path,
        } => fantome_to_project(FantomeToProjectArgs {
            fantome_path,
            output_dir,
            hashtable_path,
        }),
    }
}
