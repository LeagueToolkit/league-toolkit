use std::{
    fs::File,
    path::{Path, PathBuf},
};

use colored::Colorize;
use mod_project::ModProject;

use crate::utils;

#[derive(Debug)]
pub struct PackModProjectArgs {
    pub config_path: Option<String>,
    pub file_name: Option<String>,
    pub output_dir: String,
}

pub fn pack_mod_project(args: PackModProjectArgs) -> eyre::Result<()> {
    let config_path = resolve_config_path(args.config_path)?;

    let mod_project = load_config(&config_path)?;

    validate_layer_presence(&mod_project, &config_path)?;

    println!("Packing mod project: {}", mod_project.name.bright_yellow());

    let output_dir = PathBuf::from(&args.output_dir);
    let output_dir = match output_dir.is_absolute() {
        true => output_dir,
        false => {
            let config_dir = config_path.parent().unwrap();

            std::env::current_dir()?.join(config_dir).join(output_dir)
        }
    };

    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir)?;
    }

    Ok(())
}

// Config utils

fn resolve_config_path(config_path: Option<String>) -> eyre::Result<PathBuf> {
    match config_path {
        Some(path) => Ok(PathBuf::from(path)),
        None => {
            let cwd = std::env::current_dir()?;
            resolve_correct_config_extension(&cwd)
        }
    }
}

fn resolve_correct_config_extension(project_dir: &Path) -> eyre::Result<PathBuf> {
    // JSON first, then TOML
    let config_extensions = ["json", "toml"];

    for ext in config_extensions {
        let config_path = project_dir.join(format!("mod.config.{}", ext));
        if config_path.exists() {
            return Ok(config_path);
        }
    }

    Err(eyre::eyre!(
        "No config file found, expected mod.config.json or mod.config.toml"
            .red()
            .bold()
    ))
}

fn load_config(config_path: &Path) -> eyre::Result<ModProject> {
    let config_extension = config_path.extension().unwrap_or_default();

    match config_extension.to_str() {
        Some("json") => Ok(serde_json::from_reader(File::open(config_path)?)?),
        Some("toml") => Ok(toml::from_str(&std::fs::read_to_string(config_path)?)?),
        _ => Err(eyre::eyre!(
            "Invalid config file extension, expected mod.config.json or mod.config.toml"
        )),
    }
}

// Layer utils

fn validate_layer_presence(mod_project: &ModProject, mod_project_dir: &Path) -> eyre::Result<()> {
    if mod_project.layers.is_empty() {
        return Err(eyre::eyre!(format!(
            "No layers found in config, a {} layer is required",
            "base".bright_red().bold()
        )));
    }

    for layer in &mod_project.layers {
        if !utils::is_valid_slug(&layer.name) {
            return Err(eyre::eyre!(format!(
                "Invalid layer name: {}, must be alphanumeric and contain no spaces or special characters",
                layer.name.bright_red().bold()
            )));
        }

        if layer.name == "base" {
            return Err(eyre::eyre!(format!(
                "{} is reserved for the base layer and cannot be used as a custom layer",
                "base".bright_red().bold()
            )));
        }

        validate_layer_dir_presence(mod_project_dir, &layer.name)?;
    }

    Ok(())
}

fn validate_layer_dir_presence(mod_project_dir: &Path, layer_name: &str) -> eyre::Result<()> {
    let layer_dir = mod_project_dir.join("content").join(layer_name);
    if !layer_dir.exists() {
        return Err(eyre::eyre!(format!(
            "The directory for layer {} does not exist. Did you forget to create it?",
            layer_name.bright_red().bold()
        )));
    }

    Ok(())
}
