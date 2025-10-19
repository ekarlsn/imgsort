use iced::{task::Handle, Task};
use log::debug;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

// Global task ID counter
static TASK_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

impl TaskId {
    pub fn new() -> Self {
        TaskId(TASK_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone)]
pub enum TaskType {
    LsDir,
    PreloadImage,
}

#[derive(Debug)]
struct TaskInfo {
    task_type: TaskType,
    #[allow(dead_code)] // Used for Drop behavior to cancel tasks
    abort_handle: Handle,
}

#[derive(Debug, Default)]
pub struct TaskManager {
    active_tasks: HashMap<TaskId, TaskInfo>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            active_tasks: HashMap::new(),
        }
    }

    /// Start a cancellable task (like LsDir or PreloadImage)
    pub fn start_task<T>(
        &mut self,
        task_type: TaskType,
        future: impl std::future::Future<Output = T> + 'static + Send,
    ) -> (TaskId, Task<T>)
    where
        T: 'static + Send,
    {
        let task_id = TaskId::new();

        // Create the main task
        let main_task = Task::perform(future, |result| result);

        // Make it abortable and get the abort handle
        let (abortable_task, abort_handle) = main_task.abortable();
        let abort_on_drop_handle = abort_handle.abort_on_drop();

        // Store the task info with abort handle
        self.active_tasks.insert(
            task_id,
            TaskInfo {
                task_type: task_type.clone(),
                abort_handle: abort_on_drop_handle,
            },
        );

        debug!("Started task {:?}: {:?}", task_id, task_type);

        (task_id, abortable_task)
    }

    pub fn cancel_all(&mut self) {
        self.active_tasks.clear();
    }

    pub fn report_completed_task(&mut self, id: TaskId) -> TaskCompleteResult {
        if let Some(task_info) = self.active_tasks.remove(&id) {
            debug!("Completed task {:?}: {:?}", id, task_info.task_type);
            TaskCompleteResult::Success
        } else {
            TaskCompleteResult::TaskWasCancelled
        }
    }

    pub fn get_task_counts(&self) -> (usize, usize) {
        let mut ls_dir_count = 0;
        let mut preload_count = 0;

        for info in self.active_tasks.values() {
            match info.task_type {
                TaskType::LsDir => ls_dir_count += 1,
                TaskType::PreloadImage => preload_count += 1,
            }
        }

        (ls_dir_count, preload_count)
    }

    /// Get loading status text for UI
    pub fn get_loading_text(&self) -> String {
        let (ls_dir_count, preload_count) = self.get_task_counts();

        match (ls_dir_count > 0, preload_count > 0) {
            (true, true) => format!("Loading directory, {} images preloading...", preload_count),
            (true, false) => "Loading directory...".to_string(),
            (false, true) => format!("Loading {} images...", preload_count),
            (false, false) => "".to_string(), // No loading text when no tasks
        }
    }

    pub fn is_loading(&self) -> bool {
        !self.active_tasks.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskCompleteResult {
    Success,
    TaskWasCancelled,
}
