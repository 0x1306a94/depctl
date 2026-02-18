use anyhow::Result;
use std::path::{Path, PathBuf};
use crate::config::{self, UrlReplace};
use crate::tasks::{RepoTask, FileTask, SubRepoTask, ActionTask, LinkFileTask, CopyFileTask, TaskRunner, Task};
use crate::utils;

/// 检查路径是否匹配 skip_paths 列表中的任一配置
fn path_matches_skip_list(path: &Path, skip_paths: &[String]) -> bool {
    for skip in skip_paths {
        let skip_path = Path::new(skip.trim());
        if path.ends_with(skip_path) {
            return true;
        }
    }
    false
}

// 递归处理子仓库 DEPS 文件的任务
// 在执行时检查 DEPS 文件是否存在，这样可以处理仓库被克隆后才出现 DEPS 文件的情况
struct RecursiveDepsTask {
    deps_file: PathBuf,
    version: String,
    platform: String,
    non_recursive: bool,
    skip_paths: Vec<String>,
    url_replace_list: Option<Vec<UrlReplace>>,
    force_linkfiles: bool,
    force_copyfiles: bool,
    post_sync_stack: Option<std::sync::Arc<std::sync::Mutex<Vec<Box<dyn Task>>>>>,
}

impl RecursiveDepsTask {
    fn new(
        deps_file: PathBuf,
        _repo_dir: PathBuf,
        version: String,
        platform: String,
        non_recursive: bool,
        skip_paths: Vec<String>,
        url_replace_list: Option<Vec<UrlReplace>>,
        force_linkfiles: bool,
        force_copyfiles: bool,
        post_sync_stack: Option<std::sync::Arc<std::sync::Mutex<Vec<Box<dyn Task>>>>>,
    ) -> Self {
        Self {
            deps_file,
            version,
            platform,
            non_recursive,
            skip_paths,
            url_replace_list,
            force_linkfiles,
            force_copyfiles,
            post_sync_stack,
        }
    }
}

impl Task for RecursiveDepsTask {
    fn run(&self) -> Result<bool> {
        // 在执行时检查 DEPS 文件是否存在
        // 这样可以处理仓库被克隆后才出现 DEPS 文件的情况
        if !self.deps_file.exists() {
            return Ok(false); // DEPS 文件不存在，跳过
        }

        let task = DepsTask::new(
            self.deps_file.clone(),
            self.version.clone(),
            self.platform.clone(),
            self.non_recursive,
            self.url_replace_list.clone(),
            self.force_linkfiles,
            self.force_copyfiles,
            self.skip_paths.clone(),
            self.post_sync_stack.clone(),
        );
        task.run()?;
        Ok(true)
    }
}

pub struct DepsTask {
    config_file: PathBuf,
    version: String,
    platform: String,
    non_recursive: bool,
    url_replace_list: Option<Vec<UrlReplace>>,
    force_linkfiles: bool,
    force_copyfiles: bool,
    skip_paths: Vec<String>,
    post_sync_stack: Option<std::sync::Arc<std::sync::Mutex<Vec<Box<dyn Task>>>>>,
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
        skip_paths: Vec<String>,
        post_sync_stack: Option<std::sync::Arc<std::sync::Mutex<Vec<Box<dyn Task>>>>>,
    ) -> Self {
        Self {
            config_file,
            version,
            platform,
            non_recursive,
            url_replace_list,
            force_linkfiles,
            force_copyfiles,
            skip_paths,
            post_sync_stack,
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
            
            // 处理 submodules 和 LFS（仅在仓库需要更新或存在未完成标记时）
            if repo_dirty || unfinish_file_in_git.exists() || unfinish_file_in_root.exists() {
                tasks.push(Box::new(SubRepoTask::new(item.dir.clone())));
            }
            
            // 递归处理子仓库的 DEPS 文件（无论仓库是否需要更新，都应该检查）
            // 注意：这里不检查文件是否存在，因为仓库可能还没有被克隆
            // 我们会在执行时检查，或者在 RepoTask 执行后再检查
            if !self.non_recursive && !path_matches_skip_list(&item.dir, &self.skip_paths) {
                let deps_file = item.dir.join("DEPS");
                let item_dir = item.dir.clone();
                let version = self.version.clone();
                let platform = self.platform.clone();
                let non_recursive = self.non_recursive;
                let skip_paths = self.skip_paths.clone();
                let url_replace_list = self.url_replace_list.clone();
                let force_linkfiles = self.force_linkfiles;
                let force_copyfiles = self.force_copyfiles;
                let post_sync_stack = self.post_sync_stack.clone();
                
                // 创建一个任务，在执行时检查 DEPS 文件是否存在
                tasks.push(Box::new(RecursiveDepsTask::new(
                    deps_file,
                    item_dir,
                    version,
                    platform,
                    non_recursive,
                    skip_paths,
                    url_replace_list,
                    force_linkfiles,
                    force_copyfiles,
                    post_sync_stack,
                )));
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
        
        // 处理 linkfiles：入栈而不是立即执行（栈式执行，确保所有依赖都同步完成后再执行）
        if let Some(ref stack) = self.post_sync_stack {
            for item in &config.linkfiles {
                let mut stack_guard = stack.lock().unwrap();
                stack_guard.push(Box::new(LinkFileTask::new(item.clone(), self.force_linkfiles)));
            }
        } else {
            // 如果没有栈，直接执行（向后兼容）
            for item in &config.linkfiles {
                tasks.push(Box::new(LinkFileTask::new(item.clone(), self.force_linkfiles)));
            }
        }
        
        // 处理 copyfiles：入栈而不是立即执行
        if let Some(ref stack) = self.post_sync_stack {
            for item in &config.copyfiles {
                let mut stack_guard = stack.lock().unwrap();
                stack_guard.push(Box::new(CopyFileTask::new(item.clone(), self.force_copyfiles)));
            }
        } else {
            // 如果没有栈，直接执行（向后兼容）
            for item in &config.copyfiles {
                tasks.push(Box::new(CopyFileTask::new(item.clone(), self.force_copyfiles)));
            }
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
