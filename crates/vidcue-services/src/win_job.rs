//! Windows process-tree ownership. Unix runs each operation in its own process group and
//! signals the group; Windows has no groups, so the child goes into a job object instead.
//! Terminating (or closing) the job ends the child and everything it started, such as the
//! Python process behind a PyInstaller `yt-dlp.exe` or the FFmpeg it runs.
use std::os::windows::io::RawHandle;
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    },
};

/// `CREATE_NO_WINDOW`: console tools started from the GUI app get no console window.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct Job(HANDLE);

// The handle is an owned kernel object; it may be used and closed from any thread.
unsafe impl Send for Job {}
unsafe impl Sync for Job {}

impl Job {
    /// A kill-on-close job holding `process`. `None` when Windows refuses (for example when
    /// the process already belongs to a job that forbids nesting); the caller then falls back
    /// to killing the direct child only.
    pub fn assign(process: RawHandle) -> Option<Self> {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return None;
            }
            let job = Self(job);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) != 0;
            (configured && AssignProcessToJobObject(job.0, process as HANDLE) != 0).then_some(job)
        }
    }

    /// End every process still in the job.
    pub fn terminate(&self) {
        unsafe {
            TerminateJobObject(self.0, 1);
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // Kill-on-close: anything left in the job ends with the last handle.
        unsafe {
            CloseHandle(self.0);
        }
    }
}
