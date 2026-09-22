use crate::config::ParsedRepoItem;
use crate::tasks::Task;
use crate::utils;
use anyhow::{anyhow, bail, Result};
use std::env;
use std::path::Path;

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

fn command_error(command: &str) -> anyhow::Error {
    anyhow!("Command failed: {}", command)
}

fn run_git(dir: &Path, command: &str) -> Result<()> {
    utils::exec(command, dir, false)
}

fn is_unadvertised(captured: &utils::CapturedCommand) -> bool {
    captured.stderr.contains("unadvertised object")
        || captured.stdout.contains("unadvertised object")
}

fn commit_exists(dir: &Path, commit: &str) -> bool {
    let command = format!("git cat-file -e \"{}\"", commit);
    utils::exec_capture(&command, dir)
        .map(|captured| captured.success)
        .unwrap_or(false)
}

fn is_shallow_repository(dir: &Path) -> bool {
    utils::exec_capture("git rev-parse --is-shallow-repository", dir)
        .map(|captured| captured.success && captured.stdout.trim() == "true")
        .unwrap_or(false)
}

fn fetch_commit(dir: &Path, commit: &str) -> Result<()> {
    let shallow_command = format!("git fetch --progress --depth 1 origin \"{}\"", commit);
    let captured = utils::exec_echo(&shallow_command, dir)?;
    if captured.success {
        return Ok(());
    }
    if !is_unadvertised(&captured) {
        return Err(command_error(&shallow_command));
    }
    utils::log("【depctl】server rejected shallow fetch of commit, retrying without depth");

    let partial_command = format!(
        "git fetch --progress --filter=blob:none origin \"{}\"",
        commit
    );
    let captured = utils::exec_echo(&partial_command, dir)?;
    if captured.success {
        return Ok(());
    }
    if !is_unadvertised(&captured) {
        return Err(command_error(&partial_command));
    }
    utils::log("【depctl】server rejected fetch by commit, fetching branches and tags");

    run_git(
        dir,
        "git fetch --progress --depth 1 origin \"+refs/heads/*:refs/remotes/origin/*\" \"+refs/tags/*:refs/tags/*\"",
    )?;

    loop {
        if commit_exists(dir, commit) {
            return Ok(());
        }
        if !is_shallow_repository(dir) {
            bail!("commit {} is not reachable from any branch or tag", commit);
        }
        let shallow_file = dir.join(".git").join("shallow");
        let before = utils::read_file_opt(&shallow_file);
        utils::log("【depctl】deepening history by 100 commits");
        run_git(dir, "git fetch --progress --deepen=100 origin")?;
        if commit_exists(dir, commit) {
            return Ok(());
        }
        if utils::read_file_opt(&shallow_file) == before {
            bail!("commit {} is not reachable from any branch or tag", commit);
        }
    }
}

impl Task for RepoTask {
    fn run(&self) -> Result<bool> {
        let name = self
            .item
            .dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        utils::log(&format!(
            "【depctl】checking out repository: {}@{}",
            name, self.item.commit
        ));

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
        utils::exec(
            &format!("git remote add origin {}", url),
            &self.item.dir,
            false,
        )?;
        fetch_commit(&self.item.dir, &self.item.commit)?;

        // 恢复 LFS 备份
        if temp_lfs_dir.exists() {
            utils::move_path(&temp_lfs_dir, &lfs_bak_dir)?;
        }

        // 重置到指定 commit。回退拉分支时 FETCH_HEAD 不一定是目标 commit
        env::set_var("GIT_LFS_SKIP_SMUDGE", "1");
        run_git(
            &self.item.dir,
            &format!(
                "git reset --hard \"{}\" && git clean -df -q",
                self.item.commit
            ),
        )?;

        Ok(true) // 有输出
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "depctl-fetch-{}-{}-{}",
            name,
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed\n{}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_stdout(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed\n{}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn init_repo(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["config", "user.email", "test@example.com"]);
        git(dir, &["config", "user.name", "test"]);
        git(dir, &["config", "commit.gpgsign", "false"]);
    }

    fn commit_file(dir: &Path, name: &str, content: &str) -> String {
        fs::write(dir.join(name), content).unwrap();
        git(dir, &["add", name]);
        git(dir, &["commit", "-q", "-m", name]);
        git_stdout(dir, &["rev-parse", "HEAD"])
    }

    fn prepare_client(server: &Path, protocol_version: &str) -> PathBuf {
        let client = temp_dir("client");
        git(&client, &["init", "-q"]);
        let url = format!("file://{}", server.display());
        git(&client, &["remote", "add", "origin", &url]);
        git(&client, &["config", "protocol.version", protocol_version]);
        client
    }

    #[test]
    fn fetch_commit_keeps_shallow_clone_when_server_allows_sha() {
        let root = temp_dir("allow");
        let server = root.join("server");
        init_repo(&server);
        commit_file(&server, "a.txt", "a\n");
        let commit = commit_file(&server, "b.txt", "b\n");
        commit_file(&server, "c.txt", "c\n");
        git(
            &server,
            &["config", "uploadpack.allowReachableSHA1InWant", "true"],
        );

        let client = prepare_client(&server, "0");
        fetch_commit(&client, &commit).unwrap();

        assert!(is_shallow_repository(&client));
        assert!(commit_exists(&client, &commit));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&client);
    }

    #[test]
    fn fetch_commit_deepens_branches_when_server_rejects_unadvertised_sha() {
        let root = temp_dir("reject");
        let server = root.join("server");
        init_repo(&server);
        commit_file(&server, "a.txt", "a\n");
        let commit = commit_file(&server, "b.txt", "b\n");
        commit_file(&server, "c.txt", "c\n");
        git(
            &server,
            &["config", "uploadpack.allowReachableSHA1InWant", "false"],
        );
        git(
            &server,
            &["config", "uploadpack.allowAnySHA1InWant", "false"],
        );

        let client = prepare_client(&server, "0");
        fetch_commit(&client, &commit).unwrap();
        run_git(&client, &format!("git reset --hard \"{}\"", commit)).unwrap();

        assert_eq!(fs::read_to_string(client.join("b.txt")).unwrap(), "b\n");
        assert!(!client.join("c.txt").exists());
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&client);
    }

    #[test]
    fn fetch_commit_reports_missing_commit_when_it_is_not_on_a_branch() {
        let root = temp_dir("dangling");
        let server = root.join("server");
        init_repo(&server);
        let reachable = commit_file(&server, "a.txt", "a\n");
        let dangling = commit_file(&server, "d.txt", "d\n");
        git(&server, &["reset", "--hard", &reachable]);
        git(
            &server,
            &["config", "uploadpack.allowReachableSHA1InWant", "false"],
        );
        git(
            &server,
            &["config", "uploadpack.allowAnySHA1InWant", "false"],
        );

        let client = prepare_client(&server, "0");
        let error = fetch_commit(&client, &dangling).unwrap_err();
        assert!(error.to_string().contains("not reachable"), "{}", error);
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&client);
    }

    #[test]
    fn fetch_commit_stops_when_fetch_fails_for_another_reason() {
        let client = temp_dir("bad-remote");
        git(&client, &["init", "-q"]);
        git(
            &client,
            &["remote", "add", "origin", "/nonexistent/depctl-origin.git"],
        );

        let error = fetch_commit(&client, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("Command failed"), "{}", message);
        assert!(!message.contains("not reachable"), "{}", message);
        let _ = fs::remove_dir_all(&client);
    }
}
