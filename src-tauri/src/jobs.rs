//! The single media-job slot shared by every command.
//!
//! `JobLease` reserves the slot for one job at a time, `ActiveProcess`
//! records the running child process so `cancel_job` can stop it, and
//! `JobState` carries cancellation and per-session caches.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use crate::{job_inspector, DownloadCleanup, DownloadInfoCache, VadCache};

#[derive(Default)]
pub(crate) struct JobState {
    pub(crate) inspector: job_inspector::Inspector,
    pub(crate) active_job: Mutex<Option<String>>,
    pub(crate) cancelled: Arc<AtomicBool>,
    pub(crate) pid: Mutex<Option<u32>>,
    pub(crate) download_cleanup: Mutex<Option<DownloadCleanup>>,
    pub(crate) download_info: Mutex<Option<DownloadInfoCache>>,
    pub(crate) vad_cache: Mutex<Option<VadCache>>,
}

pub(crate) struct JobLease<'a>(&'a JobState);

impl<'a> JobLease<'a> {
    pub(crate) fn acquire(state: &'a JobState, id: Option<String>) -> Result<Self, String> {
        let mut active = state
            .active_job
            .lock()
            .map_err(|_| "Job state lock failed")?;
        if active.is_some() {
            return Err("Another media job is already running.".into());
        }
        state.cancelled.store(false, Ordering::Relaxed);
        if let Ok(mut attempts) = state.inspector.0.lock() {
            attempts.clear();
        }
        *active = Some(id.unwrap_or_else(|| {
            format!(
                "legacy-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            )
        }));
        Ok(Self(state))
    }

    /// Reserves the job slot without touching cancellation or inspector state,
    /// for maintenance work (output cleanup) that must not run beside a job.
    pub(crate) fn reserve(state: &'a JobState, id: &str) -> Result<Self, String> {
        let mut active = state
            .active_job
            .lock()
            .map_err(|_| "Job state lock failed")?;
        if active.is_some() {
            return Err("Another media job is already running.".into());
        }
        *active = Some(id.to_string());
        Ok(Self(state))
    }
}

impl Drop for JobLease<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.0.active_job.lock() {
            *active = None;
        }
    }
}

pub(crate) fn progress_job_id(state: &JobState) -> Option<String> {
    state
        .active_job
        .lock()
        .ok()
        .and_then(|active| active.clone())
}

pub(crate) fn reset_legacy_job_cancellation(state: &JobState) {
    // Command jobs reset atomically when acquiring their lease. In particular,
    // downloader setup awaits must never erase a cancellation received during
    // setup. Direct test/helper callers have no lease and retain their reset.
    if let Ok(active) = state.active_job.lock() {
        if active.is_none() {
            state.cancelled.store(false, Ordering::Relaxed);
        }
    }
}

pub(crate) struct CancelTarget {
    pub(crate) job_id: Option<String>,
    pub(crate) pid: Option<u32>,
}

pub(crate) fn request_job_cancellation(
    state: &JobState,
    job_id: Option<&str>,
) -> Result<Option<CancelTarget>, String> {
    let active = state
        .active_job
        .lock()
        .map_err(|_| "Job state lock failed")?;
    if job_id.is_some_and(|id| active.as_deref() != Some(id)) {
        return Ok(None);
    }
    let pid = *state.pid.lock().map_err(|_| "Job state lock failed")?;
    state.cancelled.store(true, Ordering::Relaxed);
    Ok(Some(CancelTarget {
        job_id: active.clone(),
        pid,
    }))
}

pub(crate) struct ActiveProcess<'a> {
    state: &'a JobState,
    pid: Option<u32>,
}

impl<'a> ActiveProcess<'a> {
    pub(crate) fn register(state: &'a JobState, pid: Option<u32>) -> Result<Self, String> {
        *state.pid.lock().map_err(|_| "Job state lock failed")? = pid;
        Ok(Self { state, pid })
    }
}

impl Drop for ActiveProcess<'_> {
    fn drop(&mut self) {
        if let Ok(mut active_pid) = self.state.pid.lock() {
            if *active_pid == self.pid {
                *active_pid = None;
            }
        }
    }
}
