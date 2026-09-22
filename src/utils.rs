use anyhow::{Context, Result};
use sha1::{Digest, Sha1};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

pub fn create_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("Failed to create directory: {}", path.display()))?;
    Ok(())
}

pub fn delete_path(path: &Path) -> Result<()> {
    if path.exists() {
        if path.is_dir() {
            fs::remove_dir_all(path)
                .with_context(|| format!("Failed to remove directory: {}", path.display()))?;
        } else {
            fs::remove_file(path)
                .with_context(|| format!("Failed to remove file: {}", path.display()))?;
        }
    }
    Ok(())
}

pub fn move_path(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    create_directory(dst.parent().unwrap())?;
    fs::rename(src, dst)
        .with_context(|| format!("Failed to move {} to {}", src.display(), dst.display()))?;
    Ok(())
}

pub fn read_file(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path.display()))
}

pub fn read_file_opt(path: &Path) -> String {
    read_file(path).unwrap_or_default()
}

pub fn write_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_directory(parent)?;
    }
    let mut file = fs::File::create(path)
        .with_context(|| format!("Failed to create file: {}", path.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("Failed to write file: {}", path.display()))?;
    Ok(())
}

pub struct CapturedCommand {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub fn exec_capture(cmd: &str, dir: &Path) -> Result<CapturedCommand> {
    exec_capture_inner(cmd, dir, false)
}

/// 执行命令，并把 stdout/stderr 原样打到终端。stderr 不是终端时 git 会藏起进度，
/// 需要进度的 git fetch 要自带 `--progress`。
pub fn exec_echo(cmd: &str, dir: &Path) -> Result<CapturedCommand> {
    exec_capture_inner(cmd, dir, true)
}

fn exec_capture_inner(cmd: &str, dir: &Path, echo: bool) -> Result<CapturedCommand> {
    let shell = if cfg!(target_os = "windows") {
        "cmd.exe"
    } else {
        "/bin/sh"
    };
    let flag = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    let mut child = Command::new(shell)
        .arg(flag)
        .arg(cmd)
        .current_dir(dir)
        .envs(std::env::vars())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to execute command: {}", cmd))?;

    let stdout_pipe = child.stdout.take().unwrap();
    let stderr_pipe = child.stderr.take().unwrap();
    let stdout_thread = thread::spawn(move || read_pipe(stdout_pipe, echo, false));
    let stderr_thread = thread::spawn(move || read_pipe(stderr_pipe, echo, true));

    let status = child
        .wait()
        .with_context(|| format!("Failed to execute command: {}", cmd))?;
    let stdout = stdout_thread.join().unwrap_or_else(|_| Vec::new());
    let stderr = stderr_thread.join().unwrap_or_else(|_| Vec::new());

    Ok(CapturedCommand {
        success: status.success(),
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

fn read_pipe(mut pipe: impl Read, echo: bool, to_stderr: bool) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                if echo {
                    if to_stderr {
                        let mut err = std::io::stderr();
                        let _ = err.write_all(&chunk[..n]);
                        let _ = err.flush();
                    } else {
                        let mut out = std::io::stdout();
                        let _ = out.write_all(&chunk[..n]);
                        let _ = out.flush();
                    }
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    buf
}

pub fn exec(cmd: &str, dir: &Path, quiet: bool) -> Result<()> {
    let shell = if cfg!(target_os = "windows") {
        "cmd.exe"
    } else {
        "/bin/sh"
    };
    let flag = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    let mut command = Command::new(shell);
    command.arg(flag).arg(cmd);
    command.current_dir(dir);
    command.envs(std::env::vars());

    if quiet {
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
    } else {
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());
    }

    let output = command
        .output()
        .with_context(|| format!("Failed to execute command: {}", cmd))?;

    if !output.status.success() {
        if quiet {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        }
        anyhow::bail!("Command failed: {}", cmd);
    }

    Ok(())
}

pub fn exec_safe(cmd: &str, dir: &Path) -> String {
    let shell = if cfg!(target_os = "windows") {
        "cmd.exe"
    } else {
        "/bin/sh"
    };
    let flag = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    let mut command = Command::new(shell);
    command.arg(flag).arg(cmd);
    command.current_dir(dir);
    command.envs(std::env::vars());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    if let Ok(output) = command.output() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        String::new()
    }
}

pub fn compare_version(version_a: &str, version_b: &str) -> i32 {
    if version_a == version_b {
        return 0;
    }

    let parts_a: Vec<u32> = version_a
        .split('.')
        .map(|s| s.parse().unwrap_or(0))
        .collect();
    let parts_b: Vec<u32> = version_b
        .split('.')
        .map(|s| s.parse().unwrap_or(0))
        .collect();

    let max_len = parts_a.len().max(parts_b.len());
    for i in 0..max_len {
        let a = parts_a.get(i).copied().unwrap_or(0);
        let b = parts_b.get(i).copied().unwrap_or(0);
        if a != b {
            return if a > b { 1 } else { -1 };
        }
    }
    0
}

pub fn get_hash(content: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn log(message: &str) {
    println!("{}", message);
}

#[allow(dead_code)]
pub fn error(message: &str) {
    eprintln!("{}", message);
}

pub fn delete_empty_dir(path: &Path) {
    if let Ok(entries) = fs::read_dir(path) {
        if entries.count() == 0 {
            let _ = fs::remove_dir(path);
        }
    }
}
