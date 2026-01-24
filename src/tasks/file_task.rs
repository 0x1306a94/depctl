use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use crate::config::ParsedFileItem;
use crate::tasks::Task;
use crate::utils;

pub struct FileTask {
    item: ParsedFileItem,
}

impl FileTask {
    pub fn new(item: ParsedFileItem) -> Self {
        Self { item }
    }
    
    fn download_file(&self, url: &str, file_path: &Path, timeout: u64) -> Result<()> {
        // 确保目录存在
        if let Some(parent) = file_path.parent() {
            utils::create_directory(parent)?;
        }
        
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(timeout))
            .build()?;
        
        let mut response = client.get(url).send()?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to download file: HTTP {}", response.status());
        }
        
        let total_size = response.content_length().unwrap_or(0);
        let mut file = File::create(file_path)
            .with_context(|| format!("Failed to create file: {}", file_path.display()))?;
        
        let pb = if total_size > 0 {
            let pb = ProgressBar::new(total_size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            pb.set_message("Downloading");
            Some(pb)
        } else {
            None
        };
        
        let mut buffer = vec![0u8; 8192];
        loop {
            let bytes_read = response.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            file.write_all(&buffer[..bytes_read])?;
            if let Some(ref progress_bar) = pb {
                progress_bar.inc(bytes_read as u64);
            }
        }
        
        if let Some(progress_bar) = pb {
            progress_bar.finish_with_message("Downloaded");
        }
        
        Ok(())
    }
    
    fn download_with_retry(&self, url: &str, file_path: &Path, timeout: u64) -> Result<()> {
        let mut retry_times = 0;
        loop {
            match self.download_file(url, file_path, timeout) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if retry_times < 3 && e.to_string().contains("timeout") {
                        retry_times += 1;
                        utils::log(&format!("Downloading retry {}: {}", retry_times, url));
                        continue;
                    }
                    return Err(e);
                }
            }
        }
    }
    
    fn unzip_file(&self, file_path: &Path, dir: &Path) -> Result<()> {
        utils::log(&format!("Unzipping: {}", file_path.display()));
        
        if file_path.to_string_lossy().ends_with(".tar.bz2") {
            self.decompress_tar_bz2(file_path, dir)?;
        } else {
            self.unzip_zip(file_path, dir)?;
        }
        
        utils::delete_path(file_path)?;
        Ok(())
    }
    
    fn unzip_zip(&self, file_path: &Path, dir: &Path) -> Result<()> {
        let file = File::open(file_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        
        // 收集根目录名称
        let mut root_names = std::collections::HashSet::new();
        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            let name = file.name();
            if let Some(first_part) = name.split('/').next() {
                if !first_part.starts_with("__MACOSX") && !name.ends_with(".DS_Store") {
                    root_names.insert(first_part.to_string());
                }
            }
        }
        
        // 删除根目录
        for root_name in &root_names {
            let target_path = dir.join(root_name);
            let _ = utils::delete_path(&target_path);
        }
        
        // 解压文件
        let file = File::open(file_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name();
            
            if name.starts_with("__MACOSX") || name.ends_with(".DS_Store") {
                continue;
            }
            
            let target_path = dir.join(name);
            
            if file.is_dir() {
                utils::create_directory(&target_path)?;
            } else {
                if let Some(parent) = target_path.parent() {
                    utils::create_directory(parent)?;
                }
                
                // 检查是否是软链接（ZIP 格式本身不支持软链接，但某些工具可能通过特殊方式存储）
                // Unix 文件系统中，如果文件模式包含 S_IFLNK (0o120000)，则可能是软链接
                #[cfg(unix)]
                {
                    use std::os::unix::fs::symlink;
                    // 检查 zip 文件的 unix_mode，如果设置了软链接标志
                    if let Some(mode) = file.unix_mode() {
                        // S_IFLNK = 0o120000 (软链接的文件类型标志)
                        if (mode & 0o170000) == 0o120000 {
                            // 读取链接目标
                            let mut link_target = String::new();
                            std::io::Read::read_to_string(&mut file, &mut link_target)?;
                            let link_target = link_target.trim();
                            
                            // 如果目标路径已存在，先删除
                            if target_path.exists() {
                                std::fs::remove_file(&target_path)?;
                            }
                            
                            // 创建软链接
                            symlink(link_target, &target_path)
                                .with_context(|| format!("Failed to create symlink: {} -> {}", target_path.display(), link_target))?;
                            continue;
                        }
                    }
                }
                
                // 普通文件
                let mut outfile = File::create(&target_path)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }
        
        Ok(())
    }
    
    fn decompress_tar_bz2(&self, file_path: &Path, output_dir: &Path) -> Result<()> {
        use bzip2::read::BzDecoder;
        use tar::Archive;
        
        let file = File::open(file_path)?;
        let decoder = BzDecoder::new(file);
        let mut archive = Archive::new(decoder);
        
        // 设置保留文件属性
        archive.set_preserve_permissions(true);
        archive.set_preserve_mtime(true);
        archive.set_unpack_xattrs(true);
        
        // 手动解压以正确处理软链接
        for entry_result in archive.entries()? {
            let mut entry = entry_result?;
            let path = entry.path()?;
            let target_path = output_dir.join(path.as_ref());
            
            // 确保父目录存在
            if let Some(parent) = target_path.parent() {
                utils::create_directory(parent)?;
            }
            
            // 检查是否是软链接
            if let Ok(Some(link_target)) = entry.link_name() {
                // 创建软链接
                #[cfg(unix)]
                {
                    use std::os::unix::fs::symlink;
                    // 如果目标路径已存在，先删除
                    if target_path.exists() {
                        std::fs::remove_file(&target_path)?;
                    }
                    // link_target 是相对路径，需要相对于目标路径解析
                    symlink(link_target.as_ref(), &target_path)
                        .with_context(|| format!("Failed to create symlink: {} -> {}", target_path.display(), link_target.display()))?;
                }
                #[cfg(not(unix))]
                {
                    // Windows 上，尝试使用 entry.unpack，它会处理软链接
                    entry.unpack(&target_path)?;
                }
            } else {
                // 普通文件或目录
                entry.unpack(&target_path)?;
            }
        }
        
        Ok(())
    }
}

impl Task for FileTask {
    fn run(&self) -> Result<bool> {
        let url_without_query: String = self.item.url.split('?').next().unwrap_or(&self.item.url).to_string();
        let path_buf = PathBuf::from(&url_without_query);
        let file_name = path_buf
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        
        utils::log(&format!("【depctl】downloading file: {}", file_name));
        
        let file_path = self.item.dir.join(file_name);
        utils::delete_path(&file_path)?;
        
        if let Some(ref multipart) = self.item.multipart {
            // 确保目录存在
            if let Some(parent) = file_path.parent() {
                utils::create_directory(parent)?;
            }
            
            // 多部分下载 - 将所有部分追加到同一个文件
            for (index, suffix) in multipart.iter().enumerate() {
                let part_url = format!("{}{}", self.item.url, suffix);
                
                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_millis(self.item.timeout))
                    .build()?;
                
                let mut response = client.get(&part_url).send()?;
                if !response.status().is_success() {
                    anyhow::bail!("Failed to download part {}: HTTP {}", index + 1, response.status());
                }
                
                let mut file = if index == 0 {
                    File::create(&file_path)
                        .with_context(|| format!("Failed to create file: {}", file_path.display()))?
                } else {
                    File::options()
                        .create(true)
                        .append(true)
                        .open(&file_path)
                        .with_context(|| format!("Failed to open file for appending: {}", file_path.display()))?
                };
                
                std::io::copy(&mut response, &mut file)?;
            }
        } else {
            // 单文件下载
            self.download_with_retry(&self.item.url, &file_path, self.item.timeout)?;
        }
        
        // 解压
        if self.item.unzip {
            self.unzip_file(&file_path, &self.item.dir)?;
        }
        
        // 写入 hash
        utils::write_file(&self.item.hash_file, &self.item.hash)?;
        
        Ok(true) // 有输出
    }
}
