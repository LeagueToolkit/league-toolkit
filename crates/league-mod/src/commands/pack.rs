use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use colored::Colorize;
use league_modpkg::{
    builder::{ModpkgBuilder, ModpkgChunkBuilder, ModpkgLayerBuilder},
    utils::hash_layer_name,
    ModpkgCompression, ModpkgMetadata,
};
use mod_project::{default_layers, ModProject, ModProjectLayer};

use crate::utils::{self, validate_mod_name, validate_version_format};

#[derive(Debug)]
pub struct PackModProjectArgs {
    pub config_path: Option<String>,
    pub file_name: Option<String>,
    pub output_dir: String,
}

pub fn pack_mod_project(args: PackModProjectArgs) -> eyre::Result<()> {
    let config_path = resolve_config_path(args.config_path)?;
    let content_dir = resolve_content_dir(&config_path)?;

    let mod_project = load_config(&config_path)?;

    validate_layer_presence(&mod_project, &content_dir)?;
    validate_mod_name(&mod_project.name)?;
    validate_version_format(&mod_project.version)?;

    println!("Packing mod project: {}", mod_project.name.bright_yellow());

    let output_dir = PathBuf::from(&args.output_dir);
    let output_dir = match output_dir.is_absolute() {
        true => output_dir,
        false => config_path.parent().unwrap().join(output_dir),
    };

    if !output_dir.exists() {
        println!("Creating output directory: {}", output_dir.display());
        std::fs::create_dir_all(&output_dir)?;
    }

    let mut modpkg_builder = ModpkgBuilder::default().with_layer(ModpkgLayerBuilder::base());
    let mut chunk_filepaths = HashMap::new();

    modpkg_builder = build_metadata(modpkg_builder, &mod_project);
    modpkg_builder = build_layers(
        modpkg_builder,
        &content_dir,
        &mod_project,
        &mut chunk_filepaths,
    )?;

    let modpkg_file_name = create_modpkg_file_name(&mod_project);
    let mut writer = BufWriter::new(File::create(output_dir.join(&modpkg_file_name))?);

    modpkg_builder.build_to_writer(&mut writer, |chunk_builder, cursor| {
        let file_path = chunk_filepaths
            .get(&(
                chunk_builder.path_hash(),
                hash_layer_name(chunk_builder.layer()),
            ))
            .unwrap();

        let mut file = File::open(file_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        cursor.write_all(&buffer)?;

        Ok(())
    })?;

    println!(
        "{}\n{}",
        "Mod package created successfully!".bright_green().bold(),
        format!("Path: {}", output_dir.join(modpkg_file_name).display()).bright_green()
    );

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

fn resolve_content_dir(config_path: &Path) -> eyre::Result<PathBuf> {
    Ok(config_path.parent().unwrap().join("content"))
}

// Layer utils

fn validate_layer_presence(mod_project: &ModProject, content_dir: &Path) -> eyre::Result<()> {
    let mut has_base = false;
    for layer in &mod_project.layers {
        if !utils::is_valid_slug(&layer.name) {
            return Err(eyre::eyre!(format!(
                "Invalid layer name: {}, must be alphanumeric and contain no spaces or special characters",
                layer.name.bright_red().bold()
            )));
        }

        if layer.name == "base" {
            has_base = true;
        }

        validate_layer_dir_presence(content_dir, &layer.name)?;
    }

    if !has_base {
        return Err(eyre::eyre!(format!(
            "{} layer is required but not found in the mod project",
            "base".bright_red().bold()
        )));
    }
    Ok(())
}

fn validate_layer_dir_presence(content_dir: &Path, layer_name: &str) -> eyre::Result<()> {
    let layer_dir = content_dir.join(layer_name);
    if !layer_dir.exists() {
        return Err(eyre::eyre!(format!(
            "The directory for layer {} does not exist. Did you forget to create it?\n{}",
            layer_name.bright_red().bold(),
            layer_dir
                .to_str()
                .unwrap_or_default()
                .bright_yellow()
                .bold()
        )));
    }

    Ok(())
}

fn build_metadata(builder: ModpkgBuilder, mod_project: &ModProject) -> ModpkgBuilder {
    builder.with_metadata(ModpkgMetadata {
        name: mod_project.name.clone(),
        display_name: mod_project.display_name.clone(),
        description: Some(mod_project.description.clone()),
        version: mod_project.version.clone(),
        distributor: None,
        authors: mod_project
            .authors
            .iter()
            .map(|a| utils::modpkg::convert_project_author(a))
            .collect(),
        license: utils::modpkg::convert_project_license(&mod_project.license),
    })
}

fn build_layers(
    mut modpkg_builder: ModpkgBuilder,
    content_dir: &Path,
    mod_project: &ModProject,
    chunk_filepaths: &mut HashMap<(u64, u64), PathBuf>,
) -> eyre::Result<ModpkgBuilder> {
    // Process base layer
    modpkg_builder = build_layer_from_dir(
        modpkg_builder,
        content_dir,
        &ModProjectLayer::base(),
        chunk_filepaths,
    )?;

    // Process layers
    for layer in &mod_project.layers {
        println!("Building layer: {}", layer.name.bright_yellow());
        modpkg_builder = modpkg_builder
            .with_layer(ModpkgLayerBuilder::new(layer.name.as_str()).with_priority(layer.priority));
        modpkg_builder = build_layer_from_dir(modpkg_builder, content_dir, layer, chunk_filepaths)?;
    }

    Ok(modpkg_builder)
}

fn build_layer_from_dir(
    mut modpkg_builder: ModpkgBuilder,
    content_dir: &Path,
    layer: &ModProjectLayer,
    chunk_filepaths: &mut HashMap<(u64, u64), PathBuf>,
) -> eyre::Result<ModpkgBuilder> {
    let layer_dir = content_dir.join(layer.name.as_str());

    for entry in glob::glob(layer_dir.join("**/*").to_str().unwrap())?.filter_map(Result::ok) {
        if !entry.is_file() {
            continue;
        }

        let layer_hash = hash_layer_name(layer.name.as_str());
        let (modpkg_builder_new, path_hash) =
            build_chunk_from_file(modpkg_builder, layer, &entry, &layer_dir)?;

        chunk_filepaths
            .entry((path_hash, layer_hash))
            .or_insert(entry);

        modpkg_builder = modpkg_builder_new;
    }

    Ok(modpkg_builder)
}

fn build_chunk_from_file(
    modpkg_builder: ModpkgBuilder,
    layer: &ModProjectLayer,
    file_path: &Path,
    layer_dir: &Path,
) -> eyre::Result<(ModpkgBuilder, u64)> {
    tracing::info!(
        "Building chunk from file: {} in layer: {}",
        file_path.display(),
        layer.name
    );

    let relative_path = file_path.strip_prefix(layer_dir)?;
    let chunk_builder = ModpkgChunkBuilder::new()
        .with_path(relative_path.to_str().unwrap())?
        .with_compression(ModpkgCompression::Zstd)
        .with_layer(layer.name.as_str());

    let path_hash = chunk_builder.path_hash();
    Ok((modpkg_builder.with_chunk(chunk_builder), path_hash))
}

fn create_modpkg_file_name(mod_project: &ModProject) -> String {
    let version = semver::Version::parse(&mod_project.version).unwrap();

    format!("{}_{}.modpkg", mod_project.name, version)
}
