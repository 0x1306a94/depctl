use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "depctl")]
#[command(about = "A dependency management tool for synchronizing project dependencies")]
pub struct CommandOptions {
    /// Platform to synchronize (mac, win, linux, or custom)
    #[arg(value_name = "PLATFORM")]
    pub platform: Option<String>,
    
    /// Print version
    #[arg(short, long)]
    pub version: bool,
    
    /// Synchronize the project in the given directory
    #[arg(short, long, value_name = "DIRECTORY")]
    pub project: Option<PathBuf>,
    
    /// Clean repos and files that do not exist in the DEPS file
    #[arg(short, long)]
    pub clean: bool,
    
    /// Skip synchronizing sub-projects
    #[arg(long)]
    pub non_recursive: bool,
    
    /// Mirror repository URLs. Format: 'old1->new1,old2->new2'
    #[arg(long, value_name = "MAPPINGS")]
    pub mirror: Option<String>,
    
    /// Force recreate linkfiles even if they already exist
    #[arg(long)]
    pub force_linkfiles: bool,
    
    /// Force recreate copyfiles even if they already exist
    #[arg(long)]
    pub force_copyfiles: bool,
}

impl CommandOptions {
    pub fn platform(&self) -> String {
        if let Some(ref platform) = self.platform {
            platform.clone()
        } else {
            detect_platform()
        }
    }
}

pub fn parse_args() -> anyhow::Result<CommandOptions> {
    let options = CommandOptions::parse();
    
    // 如果没有指定平台，自动检测
    if options.platform.is_none() {
        // platform 字段会在 platform() 方法中处理
    }
    
    Ok(options)
}

fn detect_platform() -> String {
    if cfg!(target_os = "macos") {
        "mac".to_string()
    } else if cfg!(target_os = "windows") {
        "win".to_string()
    } else if cfg!(target_os = "linux") {
        "linux".to_string()
    } else {
        "unknown".to_string()
    }
}
