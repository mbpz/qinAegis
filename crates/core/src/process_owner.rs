//! Ownership of an exclusively managed child tree. Never discovers targets to kill by PID.
//!
//! Windows references (all APIs are stable Win32):
//! https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects
//! https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags
//! https://learn.microsoft.com/en-us/windows/win32/api/tlhelp32/nf-tlhelp32-thread32first
//! https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-resumethread
//! Tokio 1.52 Child::raw_handle exposes the owned process, not its initial thread.
//! Thread enumeration is used only to pin and resume that suspended thread;
//! termination always addresses the exclusively owned job/process handles.
use std::io;
use tokio::process::{Child, Command};

pub struct ProcessTree {
    #[cfg(unix)]
    pid: u32,
    #[cfg(windows)]
    job: windows::Job,
}
impl ProcessTree {
    /// On Windows the initial thread stays suspended until it belongs to our job.
    /// Commands passed here must not depend on custom Windows creation flags.
    pub fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        command.kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        #[cfg(windows)]
        let job = windows::Job::new()?;
        #[cfg(windows)]
        command.creation_flags(0x0000_0004); // CREATE_SUSPENDED
        #[allow(unused_mut)]
        let mut child = command.spawn()?;
        #[cfg(windows)]
        if let Err(error) = job.attach_and_resume(&child) {
            // Tokio owns the exact process handle, including on assignment failure.
            let _ = child.start_kill();
            return Err(error);
        }
        let tree = Self {
            #[cfg(unix)]
            pid: child
                .id()
                .ok_or_else(|| io::Error::other("child identity unavailable"))?,
            #[cfg(windows)]
            job,
        };
        Ok((child, tree))
    }
    /// Windows success means the exclusive job is empty, not merely the root exited.
    /// A failed termination remains an error even if the root later disappears.
    pub fn cleanup_complete(&self) -> io::Result<bool> {
        #[cfg(windows)]
        {
            self.job.is_empty()
        }
        #[cfg(not(windows))]
        {
            Ok(true)
        }
    }
    pub fn terminate_now(&self, child: &mut Child) {
        #[cfg(unix)]
        if child.id() == Some(self.pid) {
            if let Ok(pid) = i32::try_from(self.pid) {
                if pid > 1 {
                    // This group was established before spawn and is still owned by
                    // our unreaped direct child. Never signal a reaped child's PID.
                    unsafe {
                        unsafe extern "C" {
                            fn kill(pid: i32, signal: i32) -> i32;
                        }
                        let _ = kill(-pid, 9);
                    }
                }
            }
        }
        #[cfg(windows)]
        let _ = self.job.terminate();
        let _ = child.start_kill();
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::ptr::null;
    use std::sync::atomic::{AtomicI32, Ordering};
    type Handle = *mut c_void;
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    impl FileTime {
        fn ticks(self) -> u64 {
            (self.high as u64) << 32 | self.low as u64
        }
    }
    #[repr(C)]
    struct BasicLimits {
        process_time: i64,
        job_time: i64,
        flags: u32,
        minimum_working_set: usize,
        maximum_working_set: usize,
        active_process_limit: u32,
        affinity: usize,
        priority: u32,
        scheduling: u32,
    }
    #[repr(C)]
    struct ExtendedLimits {
        basic: BasicLimits,
        io_counters: [u64; 6],
        process_memory: usize,
        job_memory: usize,
        peak_process_memory: usize,
        peak_job_memory: usize,
    }
    // JOBOBJECT_BASIC_ACCOUNTING_INFORMATION; ActiveProcesses is at byte 40.
    #[repr(C)]
    struct BasicAccounting {
        total_user_time: i64,
        total_kernel_time: i64,
        period_user_time: i64,
        period_kernel_time: i64,
        total_page_faults: u32,
        total_processes: u32,
        active_processes: u32,
        total_terminated: u32,
    }
    #[repr(C)]
    struct ThreadEntry {
        size: u32,
        usage: u32,
        id: u32,
        owner_pid: u32,
        base_priority: i32,
        delta_priority: i32,
        flags: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(job: Handle, class: i32, info: *const c_void, size: u32) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn TerminateJobObject(job: Handle, exit_code: u32) -> i32;
        fn QueryInformationJobObject(
            job: Handle,
            class: i32,
            info: *mut c_void,
            size: u32,
            returned: *mut u32,
        ) -> i32;
        #[cfg(test)]
        fn CreateEventW(
            attributes: *const c_void,
            manual_reset: i32,
            initial_state: i32,
            name: *const u16,
        ) -> Handle;
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
        fn Thread32First(snapshot: Handle, entry: *mut ThreadEntry) -> i32;
        fn Thread32Next(snapshot: Handle, entry: *mut ThreadEntry) -> i32;
        fn OpenThread(access: u32, inherit: i32, id: u32) -> Handle;
        fn GetProcessIdOfThread(thread: Handle) -> u32;
        fn GetProcessTimes(
            process: Handle,
            created: *mut FileTime,
            exited: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
        fn GetThreadTimes(
            thread: Handle,
            created: *mut FileTime,
            exited: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
        fn WaitForSingleObject(object: Handle, millis: u32) -> u32;
        fn ResumeThread(thread: Handle) -> u32;
    }
    pub(super) struct Job(OwnedHandle, AtomicI32);
    impl Job {
        pub(super) fn new() -> io::Result<Self> {
            // Null security attributes make the unnamed job handle non-inheritable.
            let raw = unsafe { CreateJobObjectW(null(), null()) };
            if raw.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = Self(
                unsafe { OwnedHandle::from_raw_handle(raw) },
                AtomicI32::new(0),
            );
            let mut limits: ExtendedLimits = unsafe { zeroed() };
            limits.basic.flags = 0x0000_2000; // JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                                              // No BREAKAWAY_OK/SILENT_BREAKAWAY_OK: descendants remain in this job.
            if unsafe {
                SetInformationJobObject(
                    job.0.as_raw_handle(),
                    9,
                    &limits as *const _ as *const c_void,
                    size_of::<ExtendedLimits>() as u32,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(job)
        }
        pub(super) fn attach_and_resume(&self, child: &Child) -> io::Result<()> {
            let process = child
                .raw_handle()
                .ok_or_else(|| io::Error::other("child handle unavailable"))?;
            let pid = child
                .id()
                .ok_or_else(|| io::Error::other("child identity unavailable"))?;
            if unsafe { AssignProcessToJobObject(self.0.as_raw_handle(), process) } == 0 {
                // Includes incompatible enclosing jobs. Never run unmanaged as fallback.
                return Err(io::Error::last_os_error());
            }
            let mut process_created = FileTime::default();
            let (mut exited, mut kernel, mut user) = (
                FileTime::default(),
                FileTime::default(),
                FileTime::default(),
            );
            if unsafe {
                GetProcessTimes(
                    process,
                    &mut process_created,
                    &mut exited,
                    &mut kernel,
                    &mut user,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            // Stable Tokio exposes the process handle, not the primary-thread handle.
            // Open and pin the suspended initial thread before testing its identity.
            let raw = unsafe { CreateToolhelp32Snapshot(4, 0) }; // TH32CS_SNAPTHREAD
            if raw as isize == -1 {
                return Err(io::Error::last_os_error());
            }
            let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
            let mut entry: ThreadEntry = unsafe { zeroed() };
            entry.size = size_of::<ThreadEntry>() as u32;
            let mut present = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
            while present != 0 {
                if entry.owner_pid == pid {
                    let raw_thread = unsafe { OpenThread(0x0010_0802, 0, entry.id) }; // SYNCHRONIZE | QUERY_LIMITED_INFORMATION | SUSPEND_RESUME
                    if !raw_thread.is_null() {
                        let thread = unsafe { OwnedHandle::from_raw_handle(raw_thread) };
                        let mut thread_created = FileTime::default();
                        let times_ok = unsafe {
                            GetThreadTimes(
                                thread.as_raw_handle(),
                                &mut thread_created,
                                &mut exited,
                                &mut kernel,
                                &mut user,
                            )
                        } != 0;
                        // Reject stale entries for older PID incarnations. Crucially,
                        // test the owned process is alive AFTER acquiring the thread
                        // handle; if it dies later the pinned handle cannot be reused.
                        if times_ok
                            && thread_created.ticks() >= process_created.ticks()
                            && unsafe { GetProcessIdOfThread(thread.as_raw_handle()) } == pid
                            && unsafe { WaitForSingleObject(thread.as_raw_handle(), 0) } == 0x102
                            && unsafe { WaitForSingleObject(process, 0) } == 0x102
                        {
                            if unsafe { ResumeThread(thread.as_raw_handle()) } == 1 {
                                return Ok(());
                            }
                            return Err(io::Error::other(
                                "initial child thread did not resume from suspended state",
                            ));
                        }
                    }
                }
                entry.size = size_of::<ThreadEntry>() as u32;
                present = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
            }
            Err(io::Error::other("owned suspended child thread unavailable"))
        }
        pub(super) fn terminate(&self) -> io::Result<()> {
            // Exact job handle, never taskkill /T or a snapshot of descendant PIDs.
            if unsafe { TerminateJobObject(self.0.as_raw_handle(), 1) } == 0 {
                let error = io::Error::last_os_error();
                self.1
                    .store(error.raw_os_error().unwrap_or(1), Ordering::Release);
                return Err(error);
            }
            Ok(())
        }
        pub(super) fn is_empty(&self) -> io::Result<bool> {
            let failed = self.1.load(Ordering::Acquire);
            if failed != 0 {
                return Err(io::Error::from_raw_os_error(failed));
            }
            let mut info: BasicAccounting = unsafe { zeroed() };
            let mut returned = 0;
            if unsafe {
                QueryInformationJobObject(
                    self.0.as_raw_handle(),
                    1,
                    &mut info as *mut _ as *mut c_void,
                    size_of::<BasicAccounting>() as u32,
                    &mut returned,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if returned as usize != size_of::<BasicAccounting>() {
                return Err(io::Error::other("invalid job accounting response length"));
            }
            Ok(info.active_processes == 0)
        }
    }
    #[cfg(all(test, target_pointer_width = "64"))]
    #[test]
    fn win32_struct_layout_matches_x64_abi() {
        assert_eq!(size_of::<ExtendedLimits>(), 144);
        assert_eq!(size_of::<ThreadEntry>(), 28);
        assert_eq!(size_of::<FileTime>(), 8);
        assert_eq!(size_of::<BasicAccounting>(), 48);
        assert_eq!(std::mem::offset_of!(BasicAccounting, active_processes), 40);
    }
    #[cfg(test)]
    #[test]
    fn failed_job_termination_or_accounting_never_reports_cleanup_complete() {
        let empty = Job::new().unwrap();
        assert!(empty.is_empty().unwrap());
        // A real owned event handle deliberately has the wrong kernel object type.
        // Both Win32 job APIs must report an error without closing a foreign process.
        let raw = unsafe { CreateEventW(null(), 0, 0, null()) };
        assert!(!raw.is_null());
        let invalid_job = Job(
            unsafe { OwnedHandle::from_raw_handle(raw) },
            AtomicI32::new(0),
        );
        assert!(invalid_job.is_empty().is_err());
        assert!(invalid_job.terminate().is_err());
        assert!(invalid_job.is_empty().is_err());
    }
    // OwnedHandle closes the sole job handle on Drop, terminating any survivors.
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, BufReader};
    async fn orphaned_tree() -> (Child, ProcessTree, u16) {
        let mut cmd = Command::new("python");
        cmd.args(["-u", "-c", r#"
import subprocess,sys
child=subprocess.Popen([sys.executable,'-u','-c','import socket,time; s=socket.socket(); s.bind(("127.0.0.1",0)); s.listen(); print(s.getsockname()[1],flush=True); time.sleep(120)'], stdout=subprocess.PIPE, text=True)
print(child.stdout.readline().strip(),flush=True)
"#]).stdout(std::process::Stdio::piped());
        let (mut child, tree) = ProcessTree::spawn(&mut cmd).unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let port: u16 = tokio::time::timeout(std::time::Duration::from_secs(10), lines.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .parse()
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(std::net::TcpListener::bind(("127.0.0.1", port)).is_err());
        // Child::id() is None after wait; ownership must survive that event.
        assert!(child.id().is_none());
        assert!(!tree.cleanup_complete().unwrap());
        (child, tree, port)
    }
    #[tokio::test]
    async fn job_termination_accounts_for_survivors_after_root_reaped() {
        let (mut child, tree, port) = orphaned_tree().await;
        tree.terminate_now(&mut child);
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while !tree.cleanup_complete().unwrap() {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("terminated job did not become empty");
        assert!(
            std::net::TcpListener::bind(("127.0.0.1", port)).is_ok(),
            "job reported empty before descendant released port"
        );
    }
    #[tokio::test]
    async fn job_drop_terminates_grandchild_after_direct_child_exits() {
        let (_child, tree, port) = orphaned_tree().await;
        drop(tree);
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("job close left an orphan grandchild");
    }
}
