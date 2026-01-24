use anyhow::Result;
use std::path::PathBuf;
use crate::config::{self, UrlReplace};
use crate::tasks::{RepoTask, FileTask, SubRepoTask, ActionTask, LinkFileTask, CopyFileTask, TaskRunner, Task};
use crate::utils;

pub struct DepsTask {
    config_file: PathBuf,
    version: String,
    platform: String,
    non_recursive: bool,
    url_replace_list: Option<Vec<UrlReplace>>,
    force_linkfiles: bool,
    force_copyfiles: bool,
}

impl DepsTask {
    pub fn new(
        config_file: PathBuf,
        version: String,
        platform: String,
        non_recursive: bool,
        url_replace_list: Option<Vec<UrlReplace>>,
        force_linkfiles: bool,
        force_copyfiles: bool,
    ) -> Self {
        Self {
            config_file,
            version,
            platform,
            non_recursive,
            url_replace_list,
            force_linkfiles,
            force_copyfiles,
        }
    }
    
    fn get_unfinish_file(&self) -> PathBuf {
        let project_dir = self.config_file.parent().unwrap();
        let git_dir = project_dir.join(".git");
        if git_dir.exists() && git_dir.is_dir() {
            git_dir.join(".DEPS.unfinished")
        } else {
            project_dir.join(".DEPS.unfinished")
        }
    }
}

impl Task for DepsTask {
    fn run(&self) -> Result<bool> {
        let config = config::parse(
            &self.config_file,
            &self.version,
            &self.platform,
            self.url_replace_list.as_ref(),
        )?;
        
        let mut tasks: Vec<Box<dyn Task>> = Vec::new();
        
        let unfinish_file = self.get_unfinish_file();
        let unfinish_file_in_git = self.config_file.parent().unwrap().join(".git").join(".DEPS.unfinished");
        let unfinish_file_in_root = self.config_file.parent().unwrap().join(".DEPS.unfinished");
        
        // 处理 repos
        for item in &config.repos {
            let shallow_file = item.dir.join(".git").join("shallow");
            let commit = utils::read_file_opt(&shallow_file);
            let commit = commit.trim();
            let repo_dirty = commit != item.commit;
            
            if repo_dirty {
                tasks.push(Box::new(RepoTask::new(item.clone())));
            }
            
            if repo_dirty || unfinish_file_in_git.exists() || unfinish_file_in_root.exists() {
                tasks.push(Box::new(SubRepoTask::new(item.dir.clone())));
                
                if !self.non_recursive {
                    let deps_file = item.dir.join("DEPS");
                    if deps_file.exists() {
                        tasks.push(Box::new(DepsTask::new(
                            deps_file,
                            self.version.clone(),
                            self.platform.clone(),
                            self.non_recursive,
                            self.url_replace_list.clone(),
                            self.force_linkfiles,
                            self.force_copyfiles,
                        )));
                    }
                }
            }
        }
        
        // 处理 files
        for item in &config.files {
            let cache = utils::read_file_opt(&item.hash_file);
            let cache = cache.trim();
            if cache != item.hash {
                tasks.push(Box::new(FileTask::new(item.clone())));
            }
        }
        
        // 处理主项目的 submodules 和 lfs（确保所有文件，包括 submodules 和 LFS 都已下载）
        let project_dir = self.config_file.parent().unwrap();
        tasks.push(Box::new(SubRepoTask::new(project_dir.to_path_buf())));
        
        // 处理 linkfiles（在所有依赖（包括 submodules 和 LFS）同步完成后创建软链接）
        for item in &config.linkfiles {
            tasks.push(Box::new(LinkFileTask::new(item.clone(), self.force_linkfiles)));
        }
        
        // 处理 copyfiles（在所有依赖（包括 submodules 和 LFS）同步完成后复制文件）
        for item in &config.copyfiles {
            tasks.push(Box::new(CopyFileTask::new(item.clone(), self.force_copyfiles)));
        }
        
        // 处理 actions（在 linkfiles 和 copyfiles 之后执行自定义命令）
        for item in &config.actions {
            tasks.push(Box::new(ActionTask::new(item.clone())));
        }
        
        // 写入未完成标记
        utils::write_file(&unfinish_file, "depctl is syncing...")?;
        
        // 运行所有任务
        TaskRunner::run_tasks(tasks)?;
        
        // 删除未完成标记
        let _ = utils::delete_path(&unfinish_file_in_git);
        let _ = utils::delete_path(&unfinish_file_in_root);
        
        Ok(true) // DepsTask 总是有任务执行
    }
}
