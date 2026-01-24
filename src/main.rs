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
        let version = env!("CARGO_PKG_VERSION");
        let git_hash = env!("GIT_COMMIT_HASH");
        let git_url = env!("GIT_REMOTE_URL");
        let build_time = env!("BUILD_TIME");
        
        // 只显示 commit hash 的前 7 位（短 hash）
        let short_hash = if git_hash.len() >= 7 {
            &git_hash[..7]
        } else {
            git_hash
        };
        
        if git_hash != "unknown" && git_url != "unknown" {
            println!("depctl version {} (commit {}, {}, built {})", version, short_hash, git_url, build_time);
        } else if git_hash != "unknown" {
            println!("depctl version {} (commit {}, built {})", version, short_hash, build_time);
        } else if build_time != "unknown" {
            println!("depctl version {} (built {})", version, build_time);
        } else {
            println!("depctl version {}", version);
        }
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
        // 创建全局栈，用于存储所有 linkfiles 和 copyfiles 任务
        // 在所有同步完成后，按入栈顺序执行
        let post_sync_stack = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Box<dyn tasks::Task>>::new()));
        
        let task = DepsTask::new(
            config_file,
            env!("CARGO_PKG_VERSION").to_string(),
            options.platform(),
            options.non_recursive,
            url_replace_list,
            options.force_linkfiles,
            options.force_copyfiles,
            Some(post_sync_stack.clone()),
        );
        
        // 执行所有同步任务
        task.run()?;
        
        // 所有同步完成后，执行栈中的 linkfiles 和 copyfiles（按入栈顺序）
        let mut stack_guard = post_sync_stack.lock().unwrap();
        let post_sync_tasks = std::mem::take(&mut *stack_guard);
        drop(stack_guard); // 释放锁
        
        if !post_sync_tasks.is_empty() {
            tasks::TaskRunner::run_tasks(post_sync_tasks)?;
        }
    }
    
    Ok(())
}
