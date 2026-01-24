use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // 获取 git commit hash
    let git_hash = get_git_commit_hash().unwrap_or_else(|| "unknown".to_string());
    
    // 获取 git remote URL
    let git_url = get_git_remote_url().unwrap_or_else(|| "unknown".to_string());
    
    // 获取构建时间（UTC）
    let build_time = get_build_time();
    
    // 设置环境变量，在编译时可用
    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_hash);
    println!("cargo:rustc-env=GIT_REMOTE_URL={}", git_url);
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);
    
    // 如果 git 信息变化，重新构建
    // 注意：这些文件可能不存在（非 git 仓库），但 Cargo 会忽略不存在的文件
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
    println!("cargo:rerun-if-changed=.git/config");
}

fn get_git_commit_hash() -> Option<String> {
    // 检查是否在 git 仓库中
    let git_dir_check = Command::new("git")
        .args(&["rev-parse", "--git-dir"])
        .output()
        .ok()?;
    
    if !git_dir_check.status.success() {
        return None;
    }
    
    // 尝试获取完整的 commit hash
    let output = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .ok()?;
    
    if output.status.success() {
        let hash = String::from_utf8(output.stdout).ok()?;
        Some(hash.trim().to_string())
    } else {
        None
    }
}

fn get_git_remote_url() -> Option<String> {
    // 检查是否在 git 仓库中
    let git_dir_check = Command::new("git")
        .args(&["rev-parse", "--git-dir"])
        .output()
        .ok()?;
    
    if !git_dir_check.status.success() {
        return None;
    }
    
    // 尝试获取 origin remote URL
    let output = Command::new("git")
        .args(&["config", "--get", "remote.origin.url"])
        .output()
        .ok()?;
    
    if output.status.success() {
        let url = String::from_utf8(output.stdout).ok()?;
        Some(url.trim().to_string())
    } else {
        None
    }
}

fn get_build_time() -> String {
    // 获取当前系统时间（UTC）
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    
    // 使用标准库格式化时间
    // 格式：YYYY-MM-DD HH:MM:SS UTC
    let secs = now.as_secs();
    let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, 0)
        .unwrap_or_else(|| chrono::Utc::now());
    
    datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}
