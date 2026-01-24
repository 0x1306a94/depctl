mod cli;
mod config;
mod utils;
mod tasks;

use anyhow::Result;
use tasks::{DepsTask, CleanTask, Task};

fn main() -> Result<()> {
    env_logger::init();
    
    let options = cli::parse_args()?;
    
    if options.version {
        println!("depctl version {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    
    // clap 会自动处理 --help 参数，所以这里不需要检查
    
    let config_file = if let Some(project) = &options.project {
        let deps_file = project.join("DEPS");
        if !deps_file.exists() {
            eprintln!("Cannot find DEPS file at the specified directory: {}", project.display());
            std::process::exit(1);
        }
        deps_file
    } else {
        match config::find_config_file(std::env::current_dir()?) {
            Ok(file) => file,
            Err(_) => {
                eprintln!("Cannot find DEPS file. Please run depctl in a directory with a DEPS file.");
                eprintln!("\nUse 'depctl --help' for more information.");
                std::process::exit(1);
            }
        }
    };
    
    // 解析 mirror 参数
    let url_replace_list = if let Some(ref mirror_str) = options.mirror {
        Some(config::parse_mirror(mirror_str)?)
    } else {
        None
    };
    
    if options.clean {
        let task = CleanTask::new(config_file, env!("CARGO_PKG_VERSION").to_string());
        task.run()?;
    } else {
        let task = DepsTask::new(
            config_file,
            env!("CARGO_PKG_VERSION").to_string(),
            options.platform(),
            options.non_recursive,
            url_replace_list,
            options.force_linkfiles,
            options.force_copyfiles,
        );
        task.run()?;
    }
    
    Ok(())
}
