use anyhow::{Context, Result};
use crate::config::ParsedLinkFileItem;
use crate::tasks::Task;
use crate::utils;

pub struct LinkFileTask {
    item: ParsedLinkFileItem,
    force: bool,
}

impl LinkFileTask {
    pub fn new(item: ParsedLinkFileItem, force: bool) -> Self {
        Self { item, force }
    }
}

impl Task for LinkFileTask {
    fn run(&self) -> Result<bool> {
        // 检查源文件是否存在（使用 exists() 检查，不解析软链接）
        if !self.item.src.exists() {
            anyhow::bail!("Source file does not exist: {}", self.item.src.display());
        }
        
        // 如果目标已存在且不是强制模式，跳过
        if self.item.dest.exists() && !self.force {
            return Ok(false); // 跳过，没有输出
        }
        
        // 如果目标已存在且是强制模式，先删除（可能是文件、目录或软链接）
        if self.item.dest.exists() {
            utils::delete_path(&self.item.dest)?;
        }
        
        // 确保目标路径的父目录存在
        // 例如：如果目标是 "reference/depsync"，需要确保 "reference" 目录存在
        if let Some(parent) = self.item.dest.parent() {
            if !parent.exists() {
                utils::create_directory(parent)?;
            }
        }
        
        // 创建软链接
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            
            // 计算从目标位置到源文件的相对路径
            // 这样创建的软链接更灵活，即使目录移动也能工作
            let link_target = if let Some(dest_parent) = self.item.dest.parent() {
                // 计算相对路径：从目标父目录到源文件的相对路径
                // 例如：src = "third_party/depsync", dest = "reference/depsync"
                // dest_parent = "reference", 相对路径 = "../third_party/depsync"
                pathdiff::diff_paths(&self.item.src, dest_parent)
                    .unwrap_or_else(|| {
                        // 如果无法计算相对路径，使用绝对路径
                        std::fs::canonicalize(&self.item.src)
                            .unwrap_or_else(|_| self.item.src.clone())
                    })
            } else {
                // 没有父目录，使用绝对路径
                std::fs::canonicalize(&self.item.src)
                    .unwrap_or_else(|_| self.item.src.clone())
            };
            
            // 检查源是文件还是目录
            let src_metadata = std::fs::metadata(&self.item.src)?;
            if src_metadata.is_dir() {
                // 创建目录软链接
                symlink(&link_target, &self.item.dest)
                    .with_context(|| format!("Failed to create directory symlink: {} -> {}", self.item.dest.display(), link_target.display()))?;
            } else {
                // 创建文件软链接
                symlink(&link_target, &self.item.dest)
                    .with_context(|| format!("Failed to create file symlink: {} -> {}", self.item.dest.display(), link_target.display()))?;
            }
            
            utils::log(&format!("【depctl】created symlink: {} -> {}", self.item.dest.display(), link_target.display()));
            Ok(true)
        }
        
        #[cfg(not(unix))]
        {
            // Windows 上，尝试使用 junction 或复制文件
            // 对于 Windows，我们可以创建一个硬链接或者复制文件
            // 这里简单复制文件作为后备方案
            if self.item.src.is_dir() {
                // 递归复制目录
                copy_dir_all(&self.item.src, &self.item.dest)?;
            } else {
                std::fs::copy(&self.item.src, &self.item.dest)?;
            }
            utils::log(&format!("【depctl】copied (Windows fallback): {} -> {}", self.item.dest.display(), self.item.src.display()));
            Ok(true)
        }
    }
}

#[cfg(not(unix))]
fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        
        if file_type.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    
    Ok(())
}
