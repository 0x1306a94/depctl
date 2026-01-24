mod deps_task;
mod repo_task;
mod file_task;
mod sub_repo_task;
mod action_task;
mod clean_task;
mod link_file_task;
mod copy_file_task;
mod task_runner;

pub use deps_task::DepsTask;
pub use repo_task::RepoTask;
pub use file_task::FileTask;
pub use sub_repo_task::SubRepoTask;
pub use action_task::ActionTask;
pub use clean_task::CleanTask;
pub use link_file_task::LinkFileTask;
pub use copy_file_task::CopyFileTask;
pub use task_runner::TaskRunner;

use anyhow::Result;

pub trait Task {
    /// 运行任务，返回是否实际执行了操作（有输出）
    /// true 表示任务实际执行了操作并可能有输出
    /// false 表示任务被跳过或没有实际操作
    fn run(&self) -> Result<bool>;
}
