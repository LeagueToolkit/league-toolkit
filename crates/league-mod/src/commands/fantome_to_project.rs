use crate::commands::init::prompt_mod_name;
use crate::fantome::FantomeMetadata;
use eyre::{eyre, Result};
use mod_project::{ModProject, ModProjectAuthor};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub struct FantomeToProjectArgs {
    pub fantome_path: String,
    pub output_dir: String,
}

pub fn fantome_to_project(args: FantomeToProjectArgs) -> eyre::Result<()> {
    let fantome_path = Path::new(&args.fantome_path);
    let output_dir = Path::new(&args.output_dir);

    println!("Converting fantome mod: {}", fantome_path.display());

    // Open the fantome file (which is just a renamed zip)
    let file = File::open(fantome_path)?;
    let mut archive = ZipArchive::new(file)?;

    // Extract metadata from META/info.json
    let metadata = extract_metadata(&mut archive)?;
    println!("Found mod: {} by {}", metadata.name, metadata.author);

    // Create a slug from the mod name
    let mod_name = slug::slugify(&metadata.name);

    // Create the mod project directory
    let mod_project_dir = if output_dir.exists() {
        output_dir.join(&mod_name)
    } else {
        fs::create_dir_all(output_dir)?;
        output_dir.join(&mod_name)
    };

    fs::create_dir_all(&mod_project_dir)?;
    println!("Creating mod project at: {}", mod_project_dir.display());

    // Create the content directory structure
    let content_dir = mod_project_dir.join("content");
    let base_layer_dir = content_dir.join("base");
    fs::create_dir_all(&base_layer_dir)?;

    // Create mod.config.json
    create_mod_config(&mod_project_dir, &metadata)?;

    // Process WAD files
    process_wad_files(&mut archive, &base_layer_dir)?;

    // Process RAW files
    process_raw_files(&mut archive, &base_layer_dir)?;

    println!(
        "Conversion complete! Mod project created at: {}",
        mod_project_dir.display()
    );

    Ok(())
}

fn extract_metadata(archive: &mut ZipArchive<File>) -> Result<FantomeMetadata> {
    let meta_file_path = "META/info.json";

    let mut meta_file = match archive.by_name(meta_file_path) {
        Ok(file) => file,
        Err(_) => return Err(eyre!("Could not find metadata file: {}", meta_file_path)),
    };

    let mut contents = String::new();
    meta_file.read_to_string(&mut contents)?;

    let metadata: FantomeMetadata = serde_json::from_str(&contents)?;
    Ok(metadata)
}

fn create_mod_config(mod_project_dir: &Path, metadata: &FantomeMetadata) -> Result<()> {
    let mod_project = ModProject {
        name: slug::slugify(&metadata.name),
        display_name: metadata.name.clone(),
        version: metadata.version.clone(),
        description: metadata.description.clone(),
        authors: vec![ModProjectAuthor::Name(metadata.author.clone())],
        license: None,
        transformers: vec![],
        layers: mod_project::default_layers(),
    };

    let mod_project_file_content = serde_json::to_string_pretty(&mod_project)?;
    fs::write(
        mod_project_dir.join("mod.config.json"),
        mod_project_file_content,
    )?;

    Ok(())
}

fn process_wad_files(archive: &mut ZipArchive<File>, base_layer_dir: &Path) -> Result<()> {
    // Get all WAD files
    let wad_files: Vec<String> = archive
        .file_names()
        .filter(|name| name.starts_with("WAD/") && name.ends_with(".wad"))
        .map(|s| s.to_string())
        .collect();

    if wad_files.is_empty() {
        println!("No WAD files found in the fantome package");
        return Ok(());
    }

    println!("Processing {} WAD files...", wad_files.len());

    // Look for a hashtable file
    let hashtable_path = "META/hashtable.txt";
    let mut path_map = std::collections::HashMap::new();

    if let Ok(mut hashtable_file) = archive.by_name(hashtable_path) {
        let mut contents = String::new();
        hashtable_file.read_to_string(&mut contents)?;

        // Parse the hashtable
        for line in contents.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let hash = parts[0].trim();
                let path = parts[1].trim();
                path_map.insert(hash.to_string(), path.to_string());
            }
        }

        println!("Loaded hashtable with {} entries", path_map.len());
    } else {
        println!("No hashtable found, WAD files will be extracted without path information");
    }

    // Process each WAD file
    for wad_file_path in wad_files {
        println!("Extracting WAD file: {}", wad_file_path);

        // Create a temporary directory to extract the WAD file
        let temp_dir = tempfile::tempdir()?;

        // Extract the WAD file to the temp directory
        let mut wad_file = archive.by_name(&wad_file_path)?;
        let mut wad_content = Vec::new();
        wad_file.read_to_end(&mut wad_content)?;

        let temp_wad_path = temp_dir.path().join("temp.wad");
        fs::write(&temp_wad_path, wad_content)?;

        // TODO: Use the toolkit to read and decode the WAD file
        // For now, we'll just print a message
        println!("Note: WAD file processing requires the toolkit to decode the file");
        println!("WAD file extracted to: {}", temp_wad_path.display());

        // Clean up the temp directory
        temp_dir.close()?;
    }

    Ok(())
}

fn process_raw_files(archive: &mut ZipArchive<File>, base_layer_dir: &Path) -> Result<()> {
    // Get all RAW files
    let raw_files: Vec<String> = archive
        .file_names()
        .filter(|name| name.starts_with("RAW/"))
        .map(|s| s.to_string())
        .collect();

    if raw_files.is_empty() {
        println!("No RAW files found in the fantome package");
        return Ok(());
    }

    println!("Processing {} RAW files...", raw_files.len());

    // Process each RAW file
    for raw_file_path in raw_files {
        let relative_path = raw_file_path.strip_prefix("RAW/").unwrap_or(&raw_file_path);
        let target_path = base_layer_dir.join(relative_path);

        // Create parent directories if they don't exist
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Extract the file
        let mut raw_file = archive.by_name(&raw_file_path)?;
        let mut raw_content = Vec::new();
        raw_file.read_to_end(&mut raw_content)?;

        fs::write(&target_path, raw_content)?;
        println!("Extracted: {} -> {}", raw_file_path, target_path.display());
    }

    Ok(())
}
