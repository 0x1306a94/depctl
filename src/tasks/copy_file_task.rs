use anyhow::{Context, Result};
use crate::config::ParsedCopyFileItem;
use crate::tasks::Task;
use crate::utils;

#[cfg(unix)]
use std::os::unix::fs::symlink;

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
        
        // 确保目标目录存在
        if let Some(parent) = self.item.dest.parent() {
            utils::create_directory(parent)?;
        }
        
        // 如果目标已存在且是强制模式，先删除
        if self.item.dest.exists() {
            utils::delete_path(&self.item.dest)?;
        }
        
        // 检查源是否是软链接（不跟随软链接）
        let src_metadata = std::fs::symlink_metadata(&self.item.src)?;
        let is_symlink = src_metadata.file_type().is_symlink();
        
        if is_symlink {
            // 如果源是软链接，保持软链接
            #[cfg(unix)]
            {
                // 读取软链接的目标
                let link_target = std::fs::read_link(&self.item.src)?;
                
                // 计算目标软链接的路径
                let link_target_path = if link_target.is_absolute() {
                    // 绝对路径：计算从目标位置到该绝对路径的相对路径
                    let link_target_abs = link_target.canonicalize().unwrap_or(link_target);
                    if let Some(dest_parent) = self.item.dest.parent() {
                        pathdiff::diff_paths(&link_target_abs, dest_parent)
                            .unwrap_or_else(|| {
                                // 如果无法计算相对路径，使用绝对路径
                                link_target_abs
                            })
                    } else {
                        // 没有父目录，使用绝对路径
                        link_target_abs
                    }
                } else {
                    // 相对路径：保持相对路径（相对于目标位置，即目标软链接所在目录）
                    // 例如：A/2.txt -> 1.txt，复制到 B/2.txt -> 1.txt（相对于 B）
                    link_target
                };
                
                // 检查软链接指向的是文件还是目录
                let target_metadata = std::fs::metadata(&self.item.src)?;
                if target_metadata.is_dir() {
                    // 创建目录软链接
                    symlink(&link_target_path, &self.item.dest)
                        .with_context(|| format!("Failed to create directory symlink: {} -> {}", self.item.dest.display(), link_target_path.display()))?;
                } else {
                    // 创建文件软链接
                    symlink(&link_target_path, &self.item.dest)
                        .with_context(|| format!("Failed to create file symlink: {} -> {}", self.item.dest.display(), link_target_path.display()))?;
                }
                
                utils::log(&format!("【depctl】copied symlink: {} -> {}", self.item.dest.display(), link_target_path.display()));
            }
            #[cfg(not(unix))]
            {
                // Windows 上，软链接处理较复杂，这里先复制实际内容作为后备方案
                let target_metadata = std::fs::metadata(&self.item.src)?;
                if target_metadata.is_dir() {
                    copy_dir_all(&self.item.src, &self.item.dest)?;
                } else {
                    std::fs::copy(&self.item.src, &self.item.dest)?;
                }
                utils::log(&format!("【depctl】copied (Windows fallback, symlink content): {} -> {}", self.item.src.display(), self.item.dest.display()));
            }
        } else {
            // 如果源不是软链接，检查是目录还是文件
            if src_metadata.is_dir() {
                // 递归复制目录
                copy_dir_all(&self.item.src, &self.item.dest)?;
                utils::log(&format!("【depctl】copied directory: {} -> {}", self.item.src.display(), self.item.dest.display()));
            } else {
                // 复制文件
                std::fs::copy(&self.item.src, &self.item.dest)
                    .with_context(|| format!("Failed to copy file: {} -> {}", self.item.src.display(), self.item.dest.display()))?;
                utils::log(&format!("【depctl】copied file: {} -> {}", self.item.src.display(), self.item.dest.display()));
            }
        }
        
        Ok(true)
    }
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        
        // 检查是否是软链接（不跟随软链接）
        let src_metadata = std::fs::symlink_metadata(&src_path)?;
        let is_symlink = src_metadata.file_type().is_symlink();
        
        if is_symlink {
            // 如果是软链接，保持软链接
            #[cfg(unix)]
            {
                // 读取软链接的目标
                let link_target = std::fs::read_link(&src_path)?;
                
                // 计算目标软链接的路径
                let link_target_path = if link_target.is_absolute() {
                    // 绝对路径：计算从目标位置到该绝对路径的相对路径
                    let link_target_abs = link_target.canonicalize().unwrap_or(link_target);
                    if let Some(dst_parent) = dst_path.parent() {
                        pathdiff::diff_paths(&link_target_abs, dst_parent)
                            .unwrap_or_else(|| {
                                // 如果无法计算相对路径，使用绝对路径
                                link_target_abs
                            })
                    } else {
                        // 没有父目录，使用绝对路径
                        link_target_abs
                    }
                } else {
                    // 相对路径：保持相对路径（相对于目标位置，即目标软链接所在目录）
                    // 例如：A/subdir/2.txt -> ../1.txt，复制到 B/subdir/2.txt -> ../1.txt（相对于 B/subdir）
                    link_target
                };
                
                // 检查软链接指向的是文件还是目录
                let target_metadata = std::fs::metadata(&src_path)?;
                if target_metadata.is_dir() {
                    // 创建目录软链接
                    symlink(&link_target_path, &dst_path)
                        .with_context(|| format!("Failed to create directory symlink: {} -> {}", dst_path.display(), link_target_path.display()))?;
                } else {
                    // 创建文件软链接
                    symlink(&link_target_path, &dst_path)
                        .with_context(|| format!("Failed to create file symlink: {} -> {}", dst_path.display(), link_target_path.display()))?;
                }
            }
            #[cfg(not(unix))]
            {
                // Windows 上，软链接处理较复杂，这里先复制实际内容作为后备方案
                let target_metadata = std::fs::metadata(&src_path)?;
                if target_metadata.is_dir() {
                    copy_dir_all(&src_path, &dst_path)?;
                } else {
                    std::fs::copy(&src_path, &dst_path)?;
                }
            }
        } else if src_metadata.is_dir() {
            // 普通目录，递归复制
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            // 普通文件，直接复制
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    
    Ok(())
}
