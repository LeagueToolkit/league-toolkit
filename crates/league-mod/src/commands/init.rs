use std::{
    io,
    path::{Path, PathBuf},
};

use mod_project::{ModProject, ModProjectAuthor};

use crate::utils::validate_mod_name;

#[derive(Debug, Clone)]
pub struct InitModProjectArgs {
    pub name: String,
    pub display_name: Option<String>,

    pub output_dir: Option<String>,
}

pub fn init_mod_project(args: InitModProjectArgs) -> eyre::Result<()> {
    validate_mod_name(&args.name)?;

    println!("Initializing new project: {}", args.name);

    let mod_project_dir_path = match args.output_dir {
        Some(ref output_dir) => PathBuf::from(output_dir).join(&args.name),
        None => create_mod_project_dir_path(&args.name)?,
    };

    println!(
        "Creating mod project directory at: {}",
        mod_project_dir_path.display()
    );
    std::fs::create_dir_all(&mod_project_dir_path)?;

    create_mod_project_file(&mod_project_dir_path, &args)?;

    Ok(())
}

fn create_mod_project_file(
    mod_project_dir_path: impl AsRef<Path>,
    args: &InitModProjectArgs,
) -> eyre::Result<()> {
    let mod_project =
        create_default_mod_project(Some(args.name.clone()), args.display_name.clone());

    let mod_project_file_content = serde_json::to_string(&mod_project)?;
    std::fs::write(
        mod_project_dir_path.as_ref().join("mod.config.json"),
        mod_project_file_content,
    )?;

    Ok(())
}

fn create_default_mod_project(name: Option<String>, display_name: Option<String>) -> ModProject {
    ModProject {
        name: name.unwrap_or("mod-name".to_string()),
        display_name: display_name.unwrap_or("Mod Name".to_string()),
        version: "0.1.0".to_string(),
        description: "Short description of the mod".to_string(),
        authors: vec![ModProjectAuthor::Name("<Your Name>".to_string())],
        transformers: vec![],
        layers: vec![],
    }
}

fn create_mod_project_dir_path(name: impl AsRef<Path>) -> io::Result<PathBuf> {
    Ok(std::path::Path::new(&std::env::current_dir()?).join(name))
}
