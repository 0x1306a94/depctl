use anyhow::Result;
use std::env;
use crate::config::ParsedRepoItem;
use crate::tasks::Task;
use crate::utils;

pub struct RepoTask {
    item: ParsedRepoItem,
    username: Option<String>,
    password: Option<String>,
}

impl RepoTask {
    pub fn new(item: ParsedRepoItem) -> Self {
        let username = env::var("GIT_USER").ok();
        let password = env::var("GIT_PASSWORD").ok();
        
        // 尝试从 DomainName 环境变量解析
        let (user, pass) = if username.is_none() || password.is_none() {
            if let Ok(domain_name) = env::var("DomainName") {
                let parts: Vec<&str> = domain_name.split('@').collect();
                if parts.len() == 2 {
                    let creds: Vec<&str> = parts[0].split(':').collect();
                    if creds.len() == 2 {
                        (Some(creds[0].to_string()), Some(creds[1].to_string()))
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (username.clone(), password.clone())
        };
        
        Self {
            item,
            username: user,
            password: pass,
        }
    }
    
    fn add_login_info(&self, url: &str) -> String {
        if url.contains('@') {
            return url.to_string();
        }
        
        if let (Some(ref user), Some(ref pass)) = (&self.username, &self.password) {
            if let Some(index) = url.find("://") {
                let (scheme, rest) = url.split_at(index + 3);
                return format!("{}{}:{}@{}", scheme, user, pass, rest);
            }
        }
        
        url.to_string()
    }
}

impl Task for RepoTask {
    fn run(&self) -> Result<bool> {
        let name = self.item.dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        utils::log(&format!("【depctl】checking out repository: {}@{}", name, self.item.commit));
        
        let mut url = self.item.url.clone();
        url = self.add_login_info(&url);
        
        let lfs_dir = self.item.dir.join(".git").join("lfs");
        let lfs_bak_dir = self.item.dir.join(".git").join("lfs.bak");
        let temp_lfs_dir = self.item.dir.join(".lfs.bak");
        
        // 备份 LFS 目录
        if lfs_dir.exists() {
            utils::move_path(&lfs_dir, &temp_lfs_dir)?;
        }
        
        // 删除 .git 目录
        let git_dir = self.item.dir.join(".git");
        if git_dir.exists() {
            utils::delete_path(&git_dir)?;
        }
        
        // 创建目录
        utils::create_directory(&self.item.dir)?;
        
        // 初始化 git 仓库
        utils::exec("git init -q", &self.item.dir, false)?;
        utils::exec(&format!("git remote add origin {}", url), &self.item.dir, false)?;
        utils::exec(&format!("git fetch --depth 1 origin {}", self.item.commit), &self.item.dir, false)?;
        
        // 恢复 LFS 备份
        if temp_lfs_dir.exists() {
            utils::move_path(&temp_lfs_dir, &lfs_bak_dir)?;
        }
        
        // 重置到指定 commit
        env::set_var("GIT_LFS_SKIP_SMUDGE", "1");
        utils::exec("git reset --hard FETCH_HEAD && git clean -df -q", &self.item.dir, false)?;
        
        Ok(true) // 有输出
    }
}
