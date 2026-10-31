//! One computation at a time across LyricHub's audio plugins (PLAN-plugins-2.md 5): a named mutex
//! in the session's namespace. Align and Recognize each load a model of about 0.6 GB; together
//! they would break the 1.5 GB limit. The thread that takes the lock must also drop it.

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
use windows_core::{PCWSTR, w};

pub struct ComputeLock(HANDLE);

impl ComputeLock {
    /// Waits up to `ms` for the lock; `None` while another plugin computes.
    pub fn acquire(ms: u32) -> Option<ComputeLock> {
        Self::acquire_named(w!("Local\\LyricHub.PluginCompute"), ms)
    }

    /// The test takes a lock of its own: the real one may be held by a running plugin.
    fn acquire_named(name: PCWSTR, ms: u32) -> Option<ComputeLock> {
        // SAFETY: a named mutex handle owned by this value; closed in drop or on failure.
        unsafe {
            let h = CreateMutexW(None, false, name).ok()?;
            let r = WaitForSingleObject(h, ms);
            if r == WAIT_OBJECT_0 || r == WAIT_ABANDONED {
                Some(ComputeLock(h))
            } else {
                let _ = CloseHandle(h);
                None
            }
        }
    }
}

impl Drop for ComputeLock {
    fn drop(&mut self) {
        // SAFETY: the handle is ours and owned by this thread (see the module docs).
        unsafe {
            let _ = ReleaseMutex(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_holder_waits_until_the_first_lets_go() {
        let take = |ms| ComputeLock::acquire_named(w!("Local\\LyricHub.PluginCompute.Test"), ms);
        let first = take(1000).expect("free at first");
        let other = std::thread::spawn(move || take(50).is_some()).join().unwrap();
        assert!(!other, "taken by the first holder");
        drop(first);
        let later = std::thread::spawn(move || take(1000).is_some()).join().unwrap();
        assert!(later, "free again");
    }
}
