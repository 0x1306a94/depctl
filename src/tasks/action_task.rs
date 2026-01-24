use anyhow::Result;
use crate::config::ParsedActionItem;
use crate::tasks::Task;
use crate::utils;

pub struct ActionTask {
    item: ParsedActionItem,
}

impl ActionTask {
    pub fn new(item: ParsedActionItem) -> Self {
        Self { item }
    }
}

impl Task for ActionTask {
    fn run(&self) -> Result<bool> {
        // 如果命令中包含 depsync，替换为 depctl 的绝对路径（保持兼容性）
        // 这样可以兼容原本使用 depsync 的 DEPS 文件
        let command = if self.item.command.contains("depsync") {
            // 获取当前可执行文件的绝对路径
            let exe_path = match std::env::current_exe() {
                Ok(path) => path.to_string_lossy().to_string(),
                Err(_) => "depctl".to_string(),
            };
            
            // 替换命令中的所有 depsync 为 depctl 的绝对路径
            // 例如: "depsync --clean" -> "/usr/local/bin/depctl --clean"
            //       "depsync mac" -> "/usr/local/bin/depctl mac"
            self.item.command.replace("depsync", &exe_path)
        } else {
            self.item.command.clone()
        };
        
        utils::exec(&command, &self.item.dir, false)?;
        Ok(true) // 命令执行通常有输出
    }
}
