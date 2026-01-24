use anyhow::{Context, Result};
use crate::config::ParsedCopyFileItem;
use crate::tasks::Task;
use crate::utils;

pub struct CopyFileTask {
    item: ParsedCopyFileItem,
    force: bool,
}

impl CopyFileTask {
    pub fn new(item: ParsedCopyFileItem, force: bool) -> Self {
        Self { item, force }
    }
}

impl Task for CopyFileTask {
    fn run(&self) -> Result<bool> {
        // 检查源文件是否存在
        if !self.item.src.exists() {
            anyhow::bail!("Source file does not exist: {}", self.item.src.display());
        }
        
        // 如果目标已存在且不是强制模式，跳过
        if self.item.dest.exists() && !self.force {
            return Ok(false); // 跳过，没有输出
        }
        
        // 如果源是目录，需要递归复制
        if self.item.src.is_dir() {
            // 确保目标目录存在
            if let Some(parent) = self.item.dest.parent() {
                utils::create_directory(parent)?;
            }
            
            // 如果目标已存在且是强制模式，先删除
            if self.item.dest.exists() {
                utils::delete_path(&self.item.dest)?;
            }
            
            // 递归复制目录
            copy_dir_all(&self.item.src, &self.item.dest)?;
            utils::log(&format!("【depctl】copied directory: {} -> {}", self.item.src.display(), self.item.dest.display()));
        } else {
            // 确保目标目录存在
            if let Some(parent) = self.item.dest.parent() {
                utils::create_directory(parent)?;
            }
            
            // 如果目标已存在且是强制模式，先删除
            if self.item.dest.exists() {
                utils::delete_path(&self.item.dest)?;
            }
            
            // 复制文件
            std::fs::copy(&self.item.src, &self.item.dest)
                .with_context(|| format!("Failed to copy file: {} -> {}", self.item.src.display(), self.item.dest.display()))?;
            utils::log(&format!("【depctl】copied file: {} -> {}", self.item.src.display(), self.item.dest.display()));
        }
        
        Ok(true)
    }
}

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
