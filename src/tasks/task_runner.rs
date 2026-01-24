use crate::tasks::Task;
use anyhow::Result;

pub struct TaskRunner;

impl TaskRunner {
    pub fn run_tasks(tasks: Vec<Box<dyn Task>>) -> Result<()> {
        let mut last_had_output = false;
        for task in tasks {
            let had_output = task.run()?;
            // 只在连续有输出的任务之间添加换行，避免大量空白行
            // 如果上一个任务有输出且当前任务也有输出，添加换行分隔
            if had_output {
                if last_had_output {
                    println!(); // 两个有输出的任务之间添加换行
                }
                last_had_output = true;
            } else {
                // 没有输出的任务不更新 last_had_output，这样下一个有输出的任务
                // 如果之前有输出，仍然会添加换行
                last_had_output = false;
            }
        }
        Ok(())
    }
}
