use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use crate::config;
use crate::tasks::Task;
use crate::utils;

pub struct CleanTask {
    config_file: PathBuf,
    version: String,
}

impl CleanTask {
    pub fn new(config_file: PathBuf, version: String) -> Self {
        Self {
            config_file,
            version,
        }
    }
    
    fn do_clean(
        &self,
        file_path: &Path,
        repo_paths: &[PathBuf],
        sha1_files: &[PathBuf],
    ) -> Result<()> {
        if !file_path.exists() {
            return Ok(());
        }
        
        if file_path.is_dir() {
            let git_path = file_path.join(".git");
            if git_path.exists() {
                let shallow_file = git_path.join("shallow");
                if shallow_file.exists() && !repo_paths.contains(&file_path.to_path_buf()) {
                    utils::log(&format!("【depctl】removing unused repository: {}", file_path.display()));
                    utils::delete_path(file_path)?;
                }
                return Ok(());
            }
            
            // 递归清理子目录
            if let Ok(entries) = fs::read_dir(file_path) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        self.do_clean(&entry.path(), repo_paths, sha1_files)?;
                    }
                }
            }
        } else {
            let file_name = file_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            
            if file_name.starts_with('.') && file_name.ends_with(".sha1") {
                if !sha1_files.contains(&file_path.to_path_buf()) {
                    let name = &file_name[1..file_name.len() - 5];
                    let dir_name = file_path.parent().unwrap();
                    let mut deps_file = dir_name.join(name);
                    
                    if !deps_file.exists() {
                        if name.to_lowercase().ends_with(".zip") {
                            let name_without_ext = &name[..name.len() - 4];
                            deps_file = dir_name.join(name_without_ext);
                        }
                    }
                    
                    if deps_file.exists() {
                        utils::log(&format!("【depctl】removing unused file: {}", deps_file.display()));
                        utils::delete_path(&deps_file)?;
                        utils::delete_path(file_path)?;
                        utils::delete_empty_dir(dir_name);
                    }
                }
            }
        }
        
        Ok(())
    }
}

impl Task for CleanTask {
    fn run(&self) -> Result<bool> {
        let config = config::parse(&self.config_file, &self.version, "", None)?;
        
        let deps_root = std::env::current_dir()?;
        let repo_paths: Vec<PathBuf> = config.repos.iter().map(|r| r.dir.clone()).collect();
        
        let sha1_files: Vec<PathBuf> = config.files.iter().map(|f| f.hash_file.clone()).collect();
        
        let mut had_output = false;
        if let Ok(entries) = fs::read_dir(&deps_root) {
            for entry in entries {
                if let Ok(entry) = entry {
                    // do_clean 内部会输出日志，如果有删除操作
                    self.do_clean(&entry.path(), &repo_paths, &sha1_files)?;
                    // 注意：do_clean 可能删除了文件，但我们无法直接知道
                    // 实际上 clean_task 的输出是在 do_clean 内部的 log 调用
                    // 所以这里假设总是可能有输出
                    had_output = true;
                }
            }
        }
        
        Ok(had_output)
    }
}
