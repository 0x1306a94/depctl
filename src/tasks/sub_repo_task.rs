use anyhow::Result;
use std::path::PathBuf;
use crate::tasks::Task;
use crate::utils;

pub struct SubRepoTask {
    repo_path: PathBuf,
}

impl SubRepoTask {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }
}

impl Task for SubRepoTask {
    fn run(&self) -> Result<bool> {
        let git_path = self.repo_path.join(".git");
        if !git_path.exists() {
            return Ok(false); // 没有 git 仓库，跳过
        }
        
        let mut had_output = false;
        
        // 处理 git submodules
        let modules_config = self.repo_path.join(".gitmodules");
        if modules_config.exists() {
            utils::exec(
                "git submodule update --init --recursive --depth=1",
                &self.repo_path,
                false,
            )?;
            had_output = true;
        }
        
        // 处理 git lfs
        let lfs_config = self.repo_path.join(".gitattributes");
        if lfs_config.exists() {
            let pre_push_file = self.repo_path.join(".git").join("hooks").join("pre-push");
            static mut LFS_INITIALIZED: bool = false;
            
            unsafe {
                if !LFS_INITIALIZED && !pre_push_file.exists() {
                    utils::exec("git lfs install --force", &self.repo_path, true)?;
                    LFS_INITIALIZED = true;
                }
            }
            
            let result = utils::exec_safe("git lfs fsck", &self.repo_path);
            if !result.contains("Git LFS fsck OK") {
                utils::log(&format!("【depctl】downloading git lfs objects to: {}", self.repo_path.display()));
                had_output = true;
                
                let lfs_dir = self.repo_path.join(".git").join("lfs");
                let lfs_bak_dir = self.repo_path.join(".git").join("lfs.bak");
                
                if lfs_bak_dir.exists() {
                    let _ = utils::delete_path(&lfs_dir);
                    let _ = utils::move_path(&lfs_bak_dir, &lfs_dir);
                }
                
                utils::exec("git lfs pull && git lfs prune", &self.repo_path, false)?;
            }
        }
        
        Ok(had_output)
    }
}
