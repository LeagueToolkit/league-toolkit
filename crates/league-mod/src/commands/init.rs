use std::{
    io,
    path::{Path, PathBuf},
};

use colored::Colorize;
use mod_project::{ModProject, ModProjectAuthor};

use crate::utils::{is_valid_mod_name, validate_mod_name};
use inquire::{validator::Validation, Text};

#[derive(Debug, Clone)]
pub struct InitModProjectArgs {
    pub name: Option<String>,
    pub display_name: Option<String>,

    pub output_dir: Option<String>,
}

pub fn init_mod_project(args: InitModProjectArgs) -> eyre::Result<()> {
    let display_name = match args.display_name {
        Some(ref display_name) => display_name.clone(),
        None => prompt_mod_display_name()?,
    };

    let name = match args.name {
        Some(ref name) => {
            validate_mod_name(&name)?;
            name.clone()
        }
        None => prompt_mod_name(&display_name)?,
    };

    println!("Initializing new project: {}", name.bold().bright_cyan());

    let mod_project_dir_path = match args.output_dir {
        Some(ref output_dir) => PathBuf::from(output_dir).join(&name),
        None => create_mod_project_dir_path(&name)?,
    };

    println!(
        "Creating mod project directory at: {}",
        mod_project_dir_path
            .display()
            .to_string()
            .bold()
            .bright_cyan()
    );
    std::fs::create_dir_all(&mod_project_dir_path)?;

    create_mod_project_file(&mod_project_dir_path, &name, &display_name)?;

    Ok(())
}

fn create_mod_project_file(
    mod_project_dir_path: impl AsRef<Path>,
    name: &str,
    display_name: &str,
) -> eyre::Result<()> {
    let mod_project = ModProject {
        name: name.to_string(),
        display_name: display_name.to_string(),
        version: "0.1.0".to_string(),
        description: "".to_string(),
        authors: vec![ModProjectAuthor::Name("<Your Name>".to_string())],
    };

    let mod_project_file_content = toml::to_string(&mod_project)?;
    std::fs::write(
        mod_project_dir_path.as_ref().join("modproject.toml"),
        mod_project_file_content,
    )?;

    Ok(())
}

fn create_mod_project_dir_path(name: impl AsRef<Path>) -> io::Result<PathBuf> {
    Ok(std::path::Path::new(&std::env::current_dir()?).join(name))
}

pub fn prompt_mod_name(suggested_name: impl AsRef<str>) -> eyre::Result<String> {
    let validator = |input: &str| {
        if is_valid_mod_name(input) {
            Ok(Validation::Valid)
        } else {
            Ok(Validation::Invalid(
                "Mod name must be alphanumeric and can only contain hyphens (no spaces or special characters)".into()
            ))
        }
    };

    let slugified = slug::slugify(suggested_name.as_ref());

    let name = Text::new("Enter mod folder name (no spaces or special characters):")
        .with_validator(validator)
        .with_default(&slugified)
        .with_placeholder(&slugified)
        .prompt()?;

    Ok(name)
}

fn prompt_mod_display_name() -> eyre::Result<String> {
    let name = Text::new("Enter mod display name:").prompt()?;

    Ok(name)
}
