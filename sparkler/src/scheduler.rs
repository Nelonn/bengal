/// Green Thread Scheduler
///
/// Implements a simple cooperative multitasking scheduler for green threads.
/// Threads are executed in round-robin fashion and yield control when:
/// - They explicitly call yield
/// - They perform a blocking operation (like IO)
/// - They finish execution

use crate::vm::{VM, Value, RunResult};
use std::collections::VecDeque;

/// Unique identifier for each green thread
pub type ThreadId = usize;

/// State of a green thread
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    /// Thread is ready to run
    Ready,
    /// Thread is blocked waiting for data (e.g., IO)
    Blocked,
    /// Thread has finished execution
    Finished,
}

/// A green thread with its own VM state
pub struct GreenThread {
    pub id: ThreadId,
    pub vm: VM,
    pub state: ThreadState,
    /// Data that will be used to resume the thread when unblocked
    pub resume_data: Option<Value>,
    /// Optional identifier for what this thread is waiting on (for targeted wakeups)
    pub wait_id: Option<String>,
}

impl GreenThread {
    pub fn new(id: ThreadId, vm: VM) -> Self {
        Self {
            id,
            vm,
            state: ThreadState::Ready,
            resume_data: None,
            wait_id: None,
        }
    }
}

/// Wait queue for threads waiting on data
pub struct WaitQueue {
    /// Maps data identifier to list of thread IDs waiting for it
    queues: std::collections::HashMap<String, VecDeque<ThreadId>>,
}

impl WaitQueue {
    pub fn new() -> Self {
        Self {
            queues: std::collections::HashMap::new(),
        }
    }

    /// Add a thread to the wait queue for a specific data source
    pub fn wait_for(&mut self, data_id: &str, thread_id: ThreadId) {
        self.queues
            .entry(data_id.to_string())
            .or_insert_with(VecDeque::new)
            .push_back(thread_id);
    }

    /// Wake up all threads waiting for a specific data source
    pub fn wake_all(&mut self, data_id: &str) -> Vec<ThreadId> {
        self.queues
            .remove(data_id)
            .map(|q| q.into_iter().collect())
            .unwrap_or_default()
    }

    /// Check if any threads are waiting for a data source
    pub fn has_waiters(&self, data_id: &str) -> bool {
        self.queues
            .get(data_id)
            .map(|q| !q.is_empty())
            .unwrap_or(false)
    }
}

impl Default for WaitQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Green thread scheduler
pub struct Scheduler {
    /// All threads managed by this scheduler
    threads: Vec<GreenThread>,
    /// Index of the currently running thread
    current_thread: Option<usize>,
    /// Next thread ID to assign
    next_thread_id: ThreadId,
    /// Wait queues for blocking operations
    wait_queue: WaitQueue,
    /// Maximum number of instructions to run before yielding (prevents starvation)
    instructions_per_quantum: usize,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            threads: Vec::new(),
            current_thread: None,
            next_thread_id: 0,
            wait_queue: WaitQueue::new(),
            instructions_per_quantum: 1000, // Default quantum
        }
    }

    /// Set the maximum number of instructions per time quantum
    pub fn set_quantum(&mut self, instructions: usize) {
        self.instructions_per_quantum = instructions;
    }

    /// Get the next available thread ID
    fn next_id(&mut self) -> ThreadId {
        let id = self.next_thread_id;
        self.next_thread_id += 1;
        id
    }

    /// Spawn a new green thread with the given VM
    pub fn spawn(&mut self, vm: VM) -> ThreadId {
        let id = self.next_id();
        let thread = GreenThread::new(id, vm);
        self.threads.push(thread);
        id
    }

    /// Get the current thread ID
    pub fn current_thread_id(&self) -> Option<ThreadId> {
        self.current_thread
            .map(|idx| self.threads[idx].id)
    }

    /// Get a reference to the current thread
    pub fn current_thread(&self) -> Option<&GreenThread> {
        self.current_thread
            .map(|idx| &self.threads[idx])
    }

    /// Get a mutable reference to the current thread
    pub fn current_thread_mut(&mut self) -> Option<&mut GreenThread> {
        self.current_thread
            .map(move |idx| &mut self.threads[idx])
    }

    /// Get a reference to a thread by ID
    pub fn get_thread(&self, id: ThreadId) -> Option<&GreenThread> {
        self.threads.iter().find(|t| t.id == id)
    }

    /// Get a mutable reference to a thread by ID
    pub fn get_thread_mut(&mut self, id: ThreadId) -> Option<&mut GreenThread> {
        self.threads.iter_mut().find(|t| t.id == id)
    }

    /// Mark the current thread as blocked waiting for data
    pub fn block_current(&mut self, data_id: &str) {
        if let Some(id) = self.current_thread_id() {
            if let Some(thread) = self.get_thread_mut(id) {
                thread.state = ThreadState::Blocked;
            }
            self.wait_queue.wait_for(data_id, id);
        }
    }

    /// Wake up threads waiting for data and mark them as ready
    pub fn wake_waiters(&mut self, data_id: &str) -> Vec<ThreadId> {
        let awakened = self.wait_queue.wake_all(data_id);
        for id in &awakened {
            if let Some(thread) = self.get_thread_mut(*id) {
                thread.state = ThreadState::Ready;
            }
        }
        awakened
    }

    /// Set resume data for a thread
    pub fn set_resume_data(&mut self, id: ThreadId, data: Value) {
        if let Some(thread) = self.get_thread_mut(id) {
            thread.resume_data = Some(data);
        }
    }

    /// Wake up all blocked threads with resume data
    pub fn wake_all_blocked(&mut self, data: Value) {
        for thread in &mut self.threads {
            if thread.state == ThreadState::Blocked {
                thread.state = ThreadState::Ready;
                thread.resume_data = Some(data.clone());
                thread.wait_id = None;
            }
        }
    }

    /// Wake up a specific blocked thread by wait_id
    pub fn wake_by_wait_id(&mut self, wait_id: &str, data: Value) {
        for thread in &mut self.threads {
            if thread.state == ThreadState::Blocked && thread.wait_id.as_deref() == Some(wait_id) {
                thread.state = ThreadState::Ready;
                thread.resume_data = Some(data);
                thread.wait_id = None;
                return;
            }
        }
    }

    /// Yield execution to the next ready thread
    /// Returns the ID of the next thread to run, or None if no threads are ready
    pub fn yield_to_next(&mut self) -> Option<ThreadId> {
        let current_idx = self.current_thread?;
        
        // Mark current as ready if it's not blocked or finished
        if let Some(thread) = self.threads.get(current_idx) {
            if thread.state == ThreadState::Ready {
                // Still ready, will continue from where we left off later
            }
        }

        // Find next ready thread
        let next_idx = self.find_next_ready_thread(current_idx)?;
        self.current_thread = Some(next_idx);
        Some(self.threads[next_idx].id)
    }

    /// Find the next ready thread starting from current position
    fn find_next_ready_thread(&self, start_idx: usize) -> Option<usize> {
        let len = self.threads.len();
        if len == 0 {
            return None;
        }

        // Search from next position
        for i in 1..=len {
            let idx = (start_idx + i) % len;
            if let Some(thread) = self.threads.get(idx) {
                if thread.state == ThreadState::Ready {
                    return Some(idx);
                }
            }
        }
        None
    }

    /// Find the next ready thread starting from a specific index
    fn find_next_ready_thread_from(&self, start_idx: usize) -> Option<usize> {
        let len = self.threads.len();
        if len == 0 {
            return None;
        }

        // Search from start position (inclusive)
        for i in 0..len {
            let idx = (start_idx + i) % len;
            if let Some(thread) = self.threads.get(idx) {
                if thread.state == ThreadState::Ready {
                    return Some(idx);
                }
            }
        }
        None
    }

    /// Run the scheduler until all threads finish or we hit a blocking operation
    /// Returns (result, has_blocked_threads)
    pub fn run(&mut self) -> (Option<Value>, bool) {
        // Find first ready thread, starting from next thread after current for round-robin
        let start_search_from = self.current_thread.map(|i| (i + 1) % self.threads.len()).unwrap_or(0);
        let start_idx = self.find_next_ready_thread_from(start_search_from);

        if let Some(idx) = start_idx {
            self.current_thread = Some(idx);
        } else {
            // No ready threads
            let has_blocked = self.threads.iter().any(|t| t.state == ThreadState::Blocked);
            return (None, has_blocked);
        }

        // Run threads in round-robin
        let mut instructions_run = 0;
        let mut last_result = None;

        loop {
            // Check if all threads are finished
            if !self.threads.iter().any(|t| t.state == ThreadState::Ready) {
                let has_blocked = self.threads.iter().any(|t| t.state == ThreadState::Blocked);

                // If all finished, return the last result
                if !has_blocked {
                    return (last_result, false);
                }

                // We have blocked threads, return and wait for external wake
                return (None, true);
            }

            // Run current thread
            if let Some(current_idx) = self.current_thread {
                if let Some(thread) = self.threads.get_mut(current_idx) {
                    if thread.state != ThreadState::Ready {
                        // Move to next thread
                        if self.yield_to_next().is_none() {
                            break;
                        }
                        continue;
                    }

                    // Resume with data if available
                    if let Some(data) = thread.resume_data.take() {
                        let _ = thread.vm.resume_with_result(Ok(data));
                    }

                    // Run the VM
                    match thread.vm.run() {
                        Ok(result) => {
                            instructions_run += 1;

                            match result {
                                RunResult::Finished(val) => {
                                    thread.state = ThreadState::Finished;
                                    last_result = val;

                                    // Move to next thread
                                    if self.yield_to_next().is_none() {
                                        break;
                                    }
                                }
                                RunResult::Suspended => {
                                    // Thread suspended for async operation
                                    // Mark as blocked and move to next
                                    thread.state = ThreadState::Blocked;
                                    // Set wait_id if the VM has one pending
                                    if let Some(wait_id) = thread.vm.context.pending_wait_id.take() {
                                        thread.wait_id = Some(wait_id);
                                    }

                                    if self.yield_to_next().is_none() {
                                        break;
                                    }
                                }
                                RunResult::Breakpoint => {
                                    // Hit a breakpoint, yield
                                    if self.yield_to_next().is_none() {
                                        break;
                                    }
                                }
                                RunResult::InProgress => {
                                    // Continue running if we haven't exceeded quantum
                                    if instructions_run >= self.instructions_per_quantum {
                                        // Time slice exceeded, yield to next thread
                                        instructions_run = 0;
                                        if self.yield_to_next().is_none() {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Thread {} error: {:?}", thread.id, e);
                            thread.state = ThreadState::Finished;

                            if self.yield_to_next().is_none() {
                                break;
                            }
                        }
                    }
                }
            }
        }

        let has_blocked = self.threads.iter().any(|t| t.state == ThreadState::Blocked);
        (last_result, has_blocked)
    }

    /// Get the number of active (non-finished) threads
    pub fn active_thread_count(&self) -> usize {
        self.threads.iter()
            .filter(|t| t.state != ThreadState::Finished)
            .count()
    }

    /// Get the number of ready threads
    pub fn ready_thread_count(&self) -> usize {
        self.threads.iter()
            .filter(|t| t.state == ThreadState::Ready)
            .count()
    }

    /// Get the number of blocked threads
    pub fn blocked_thread_count(&self) -> usize {
        self.threads.iter()
            .filter(|t| t.state == ThreadState::Blocked)
            .count()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}
