//! # ITK Sync
//!
//! Clock synchronization and drift correction for multiplayer scenarios.
//!
//! This crate provides:
//! - NTP-lite clock offset estimation
//! - Reference-point based position calculation
//! - Drift correction with smooth rate adjustment
//!
//! ## Sync Model
//!
//! Rather than constantly syncing "current position", we sync a reference point:
//!
//! ```text
//! SyncState {
//!     position_at_ref_ms: u64,    // Position at reference time
//!     ref_wallclock_ms: u64,      // When that position was valid
//!     is_playing: bool,
//!     playback_rate: f64,         // Usually 1.0
//! }
//! ```
//!
//! Each client computes current position locally:
//! ```text
//! current_pos = position_at_ref + (now - ref_time) * rate
//! ```

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
use std::time::Duration;
use thiserror::Error;
use tracing::warn;

/// Sync errors
#[derive(Error, Debug)]
pub enum SyncError {
    #[error("clock offset not yet estimated")]
    NoClockOffset,

    #[error("insufficient samples for estimation")]
    InsufficientSamples,

    #[error("reference time is in the future")]
    FutureReference,

    #[error("time conversion overflow: result would be negative or exceed u64::MAX")]
    TimeConversionOverflow,
}

/// Result type for sync operations
pub type Result<T> = std::result::Result<T, SyncError>;

/// Get current time in milliseconds since UNIX epoch
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Clock synchronization state
///
/// Uses NTP-lite algorithm to estimate offset between local and remote clocks.
pub struct ClockSync {
    /// Estimated offset: remote_time = local_time + offset
    offset_ms: Option<i64>,

    /// Recent RTT samples for filtering
    samples: VecDeque<ClockSample>,

    /// Maximum samples to keep
    max_samples: usize,
}

#[derive(Debug, Clone, Copy)]
struct ClockSample {
    offset_ms: i64,
    rtt_ms: u64,
}

impl ClockSync {
    /// Create a new clock sync instance
    pub fn new() -> Self {
        Self {
            offset_ms: None,
            samples: VecDeque::with_capacity(10),
            max_samples: 10,
        }
    }

    /// Process a ping/pong exchange
    ///
    /// # Arguments
    /// * `send_time_ms` - Local time when ping was sent
    /// * `remote_time_ms` - Remote time when pong was created
    /// * `recv_time_ms` - Local time when pong was received
    pub fn process_pong(&mut self, send_time_ms: u64, remote_time_ms: u64, recv_time_ms: u64) {
        let rtt_ms = recv_time_ms.saturating_sub(send_time_ms);
        let one_way_ms = rtt_ms / 2;

        // Estimate: at what local time was remote_time_ms?
        // Answer: send_time_ms + one_way_ms
        // So: offset = remote_time_ms - (send_time_ms + one_way_ms)
        let estimated_local_at_remote = send_time_ms + one_way_ms;
        let offset_ms = remote_time_ms as i64 - estimated_local_at_remote as i64;

        let sample = ClockSample { offset_ms, rtt_ms };

        // Add sample
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);

        // Update estimated offset using median of recent samples
        // (median is more robust to outliers than mean)
        self.update_offset();
    }

    fn update_offset(&mut self) {
        if self.samples.is_empty() {
            return;
        }

        // Use samples with lowest RTT (most reliable)
        let mut sorted: Vec<_> = self.samples.iter().collect();
        sorted.sort_by_key(|s| s.rtt_ms);

        // Take median of best half
        let best_count = sorted.len().div_ceil(2);
        let best: Vec<_> = sorted.into_iter().take(best_count).collect();

        // Median offset
        let mut offsets: Vec<_> = best.iter().map(|s| s.offset_ms).collect();
        offsets.sort();

        let median_offset = if offsets.len() % 2 == 0 {
            (offsets[offsets.len() / 2 - 1] + offsets[offsets.len() / 2]) / 2
        } else {
            offsets[offsets.len() / 2]
        };

        self.offset_ms = Some(median_offset);
    }

    /// Get estimated clock offset
    ///
    /// `remote_time = local_time + offset`
    pub fn offset_ms(&self) -> Option<i64> {
        self.offset_ms
    }

    /// Convert local time to estimated remote time
    ///
    /// Returns an error if the result would overflow or underflow.
    pub fn local_to_remote(&self, local_ms: u64) -> Result<u64> {
        let offset = self.offset_ms.ok_or(SyncError::NoClockOffset)?;

        // Use checked arithmetic to handle both positive and negative offsets
        if offset >= 0 {
            local_ms
                .checked_add(offset as u64)
                .ok_or(SyncError::TimeConversionOverflow)
        } else {
            let abs_offset = offset.unsigned_abs();
            local_ms
                .checked_sub(abs_offset)
                .ok_or(SyncError::TimeConversionOverflow)
        }
    }

    /// Convert remote time to estimated local time
    ///
    /// Returns an error if the result would overflow or underflow.
    pub fn remote_to_local(&self, remote_ms: u64) -> Result<u64> {
        let offset = self.offset_ms.ok_or(SyncError::NoClockOffset)?;

        // Use checked arithmetic to handle both positive and negative offsets
        if offset >= 0 {
            remote_ms
                .checked_sub(offset as u64)
                .ok_or(SyncError::TimeConversionOverflow)
        } else {
            let abs_offset = offset.unsigned_abs();
            remote_ms
                .checked_add(abs_offset)
                .ok_or(SyncError::TimeConversionOverflow)
        }
    }

    /// Check if clock is synchronized
    pub fn is_synced(&self) -> bool {
        self.offset_ms.is_some()
    }

    /// Clear all samples and reset
    pub fn reset(&mut self) {
        self.offset_ms = None;
        self.samples.clear();
    }
}

impl Default for ClockSync {
    fn default() -> Self {
        Self::new()
    }
}

/// Playback synchronization state
#[derive(Debug, Clone)]
pub struct PlaybackSync {
    /// Content identifier (URL, file hash, etc.)
    pub content_id: String,

    /// Position at reference time (milliseconds into content)
    pub position_at_ref_ms: u64,

    /// Reference wallclock time (milliseconds since epoch)
    pub ref_wallclock_ms: u64,

    /// Whether currently playing
    pub is_playing: bool,

    /// Playback rate (1.0 = normal, 0.95-1.05 for drift correction)
    pub playback_rate: f64,
}

impl PlaybackSync {
    /// Create a new playback sync state
    pub fn new(content_id: String) -> Self {
        Self {
            content_id,
            position_at_ref_ms: 0,
            ref_wallclock_ms: now_ms(),
            is_playing: false,
            playback_rate: 1.0,
        }
    }

    /// Calculate current position based on reference point
    pub fn current_position_ms(&self) -> u64 {
        if !self.is_playing {
            return self.position_at_ref_ms;
        }

        let now = now_ms();
        let elapsed = now.saturating_sub(self.ref_wallclock_ms);

        // Detect backward clock jumps (ref_wallclock is in the future)
        if self.ref_wallclock_ms > now && self.ref_wallclock_ms - now > 1000 {
            warn!(
                ref_wallclock_ms = self.ref_wallclock_ms,
                now_ms = now,
                jump_ms = self.ref_wallclock_ms - now,
                "clock jump detected: reference time is in the future"
            );
        }

        let adjusted_elapsed = (elapsed as f64 * self.playback_rate) as u64;

        // Use saturating_add to prevent overflow on large values
        self.position_at_ref_ms.saturating_add(adjusted_elapsed)
    }

    /// Update from a received sync state (e.g., from network)
    pub fn update_from(&mut self, other: &PlaybackSync) {
        self.content_id = other.content_id.clone();
        self.position_at_ref_ms = other.position_at_ref_ms;
        self.ref_wallclock_ms = other.ref_wallclock_ms;
        self.is_playing = other.is_playing;
        // Don't copy playback_rate - that's local drift correction
    }

    /// Set playing state
    pub fn set_playing(&mut self, playing: bool) {
        // When changing state, update reference point to current position
        self.position_at_ref_ms = self.current_position_ms();
        self.ref_wallclock_ms = now_ms();
        self.is_playing = playing;
    }

    /// Seek to a position
    pub fn seek(&mut self, position_ms: u64) {
        self.position_at_ref_ms = position_ms;
        self.ref_wallclock_ms = now_ms();
    }
}

/// Drift correction calculator
pub struct DriftCorrector {
    /// Target position (from sync leader)
    target_sync: Option<PlaybackSync>,

    /// Clock sync for time conversion
    clock_sync: ClockSync,
}

impl DriftCorrector {
    /// Create a new drift corrector
    pub fn new() -> Self {
        Self {
            target_sync: None,
            clock_sync: ClockSync::new(),
        }
    }

    /// Update target sync state from leader
    pub fn update_target(&mut self, sync: PlaybackSync) {
        self.target_sync = Some(sync);
    }

    /// Get the clock sync instance for processing pongs
    pub fn clock_sync_mut(&mut self) -> &mut ClockSync {
        &mut self.clock_sync
    }

    /// Calculate recommended playback rate to correct drift
    ///
    /// Returns the rate adjustment (1.0 = no change)
    pub fn calculate_rate(&self, current_position_ms: u64) -> f64 {
        let Some(target) = &self.target_sync else {
            return 1.0;
        };

        if !target.is_playing {
            return 1.0;
        }

        // Calculate target position, correcting for clock offset between local and remote
        let target_position = self.target_position_ms(target);

        // Calculate drift (positive = we're ahead, negative = we're behind)
        let drift_ms = current_position_ms as i64 - target_position as i64;

        // Apply correction based on drift magnitude
        match drift_ms.abs() {
            0..=150 => 1.0, // Within tolerance, no correction
            151..=500 => {
                // Gentle correction
                if drift_ms > 0 { 0.98 } else { 1.02 }
            },
            501..=1500 => {
                // Moderate correction
                if drift_ms > 0 { 0.95 } else { 1.05 }
            },
            _ => {
                // Large drift - recommend hard seek instead
                0.0
            },
        }
    }

    /// Check if a hard seek is recommended (drift too large)
    pub fn should_seek(&self, current_position_ms: u64) -> Option<u64> {
        let Some(target) = &self.target_sync else {
            return None;
        };

        let target_position = self.target_position_ms(target);
        let drift_ms = (current_position_ms as i64 - target_position as i64).abs();

        if drift_ms > 1500 {
            Some(target_position)
        } else {
            None
        }
    }

    /// Get current drift in milliseconds (positive = ahead, negative = behind)
    pub fn current_drift_ms(&self, current_position_ms: u64) -> Option<i64> {
        let target = self.target_sync.as_ref()?;
        let target_position = self.target_position_ms(target);
        Some(current_position_ms as i64 - target_position as i64)
    }

    /// Calculate target position corrected for clock offset.
    ///
    /// The target's `ref_wallclock_ms` is in the remote clock's time domain.
    /// We convert it to local time using the estimated clock offset before
    /// computing elapsed time since the reference point.
    fn target_position_ms(&self, target: &PlaybackSync) -> u64 {
        if !target.is_playing {
            return target.position_at_ref_ms;
        }

        // Convert remote reference wallclock to local time using clock offset.
        // If no offset is available yet, fall back to using the remote time directly
        // (which may be slightly inaccurate but avoids blocking on sync).
        let local_ref_time = match self.clock_sync.remote_to_local(target.ref_wallclock_ms) {
            Ok(local_time) => local_time,
            Err(_) => target.ref_wallclock_ms,
        };

        let elapsed = now_ms().saturating_sub(local_ref_time);
        let adjusted_elapsed = (elapsed as f64 * target.playback_rate) as u64;

        target.position_at_ref_ms.saturating_add(adjusted_elapsed)
    }
}

impl Default for DriftCorrector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_sync_basic() {
        let mut sync = ClockSync::new();

        // Simulate ping/pong with 100ms RTT and 50ms offset
        // local send: 1000, remote: 1050, local recv: 1100
        sync.process_pong(1000, 1050, 1100);

        // Offset should be approximately 0 (50 - 50 = 0)
        // local_at_remote = 1000 + 50 = 1050
        // offset = 1050 - 1050 = 0
        assert!(sync.is_synced());
        assert_eq!(sync.offset_ms(), Some(0));
    }

    #[test]
    fn test_playback_sync_position() {
        let mut sync = PlaybackSync::new("test".to_string());
        sync.position_at_ref_ms = 10000;
        sync.ref_wallclock_ms = now_ms() - 1000; // 1 second ago
        sync.is_playing = true;
        sync.playback_rate = 1.0;

        // Should be approximately 11000 (10000 + 1000)
        let pos = sync.current_position_ms();
        assert!((10900..=11100).contains(&pos));
    }

    #[test]
    fn test_drift_correction_rates() {
        let corrector = DriftCorrector::new();

        // No target = no correction
        assert_eq!(corrector.calculate_rate(5000), 1.0);
    }

    #[test]
    fn test_playback_sync_paused() {
        let mut sync = PlaybackSync::new("test".to_string());
        sync.position_at_ref_ms = 10000;
        sync.is_playing = false;

        // Paused - position doesn't change
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(sync.current_position_ms(), 10000);
    }

    // =========================================================================
    // Clock sync edge case tests
    // =========================================================================

    #[test]
    fn test_clock_sync_convergence_with_noisy_samples() {
        let mut sync = ClockSync::new();

        // Simulate multiple samples with varying RTTs and a true offset of ~50ms.
        // Pair: (send, remote, recv) -> offset = remote - (send + rtt/2)
        let samples = [
            // Low RTT, accurate
            (1000u64, 1060u64, 1020u64), // rtt=20, offset=50
            (1100, 1160, 1120),          // rtt=20, offset=50
            // Higher RTT, slightly noisy
            (1200, 1310, 1400), // rtt=200, offset=10 (noisy)
            (1300, 1360, 1320), // rtt=20, offset=50
            (1400, 1460, 1420), // rtt=20, offset=50
        ];

        for (send, remote, recv) in samples {
            sync.process_pong(send, remote, recv);
        }

        // Median-of-best-half should converge near 50ms offset
        let offset = sync.offset_ms().unwrap();
        assert!(
            (40..=60).contains(&offset),
            "Expected offset near 50, got {offset}"
        );
    }

    #[test]
    fn test_clock_sync_local_to_remote_overflow() {
        let mut sync = ClockSync::new();
        // offset = t2 - t1 - (t3-t1)/2 = 1100 - 1000 - 50 = +50
        sync.process_pong(1000, 1100, 1100);

        // Positive offset: local near u64::MAX should overflow
        let result = sync.local_to_remote(u64::MAX);
        assert!(result.is_err());
    }

    #[test]
    fn test_clock_sync_remote_to_local_underflow() {
        let mut sync = ClockSync::new();
        // Create a large positive offset
        sync.process_pong(0, 1000, 100);

        // Try to convert a small remote time - should underflow
        let result = sync.remote_to_local(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_clock_sync_reset() {
        let mut sync = ClockSync::new();
        sync.process_pong(1000, 1050, 1100);
        assert!(sync.is_synced());

        sync.reset();
        assert!(!sync.is_synced());
        assert_eq!(sync.offset_ms(), None);
    }

    // =========================================================================
    // Drift correction tests
    // =========================================================================

    #[test]
    fn test_drift_correction_within_tolerance() {
        let mut corrector = DriftCorrector::new();
        let mut target = PlaybackSync::new("test".to_string());
        target.position_at_ref_ms = 10000;
        target.ref_wallclock_ms = now_ms();
        target.is_playing = true;
        target.playback_rate = 1.0;

        corrector.update_target(target);

        // Position within 150ms tolerance: no correction
        let rate = corrector.calculate_rate(10100);
        assert_eq!(rate, 1.0);
    }

    #[test]
    fn test_drift_correction_gentle_ahead() {
        let mut corrector = DriftCorrector::new();
        let mut target = PlaybackSync::new("test".to_string());
        target.position_at_ref_ms = 10000;
        target.ref_wallclock_ms = now_ms();
        target.is_playing = true;
        target.playback_rate = 1.0;

        corrector.update_target(target);

        // 300ms ahead: gentle slowdown
        let rate = corrector.calculate_rate(10300);
        assert_eq!(rate, 0.98);
    }

    #[test]
    fn test_drift_correction_gentle_behind() {
        let mut corrector = DriftCorrector::new();
        let mut target = PlaybackSync::new("test".to_string());
        target.position_at_ref_ms = 10000;
        target.ref_wallclock_ms = now_ms();
        target.is_playing = true;
        target.playback_rate = 1.0;

        corrector.update_target(target);

        // 300ms behind: gentle speedup
        let rate = corrector.calculate_rate(9700);
        assert_eq!(rate, 1.02);
    }

    #[test]
    fn test_drift_correction_moderate() {
        let mut corrector = DriftCorrector::new();
        let mut target = PlaybackSync::new("test".to_string());
        target.position_at_ref_ms = 10000;
        target.ref_wallclock_ms = now_ms();
        target.is_playing = true;
        target.playback_rate = 1.0;

        corrector.update_target(target);

        // 1000ms ahead: moderate slowdown
        let rate = corrector.calculate_rate(11000);
        assert_eq!(rate, 0.95);

        // 1000ms behind: moderate speedup
        let rate = corrector.calculate_rate(9000);
        assert_eq!(rate, 1.05);
    }

    #[test]
    fn test_drift_correction_large_requires_seek() {
        let mut corrector = DriftCorrector::new();
        let mut target = PlaybackSync::new("test".to_string());
        target.position_at_ref_ms = 10000;
        target.ref_wallclock_ms = now_ms();
        target.is_playing = true;
        target.playback_rate = 1.0;

        corrector.update_target(target);

        // 2000ms drift: recommend seek (rate = 0.0)
        let rate = corrector.calculate_rate(12000);
        assert_eq!(rate, 0.0);

        // should_seek returns target position
        let seek_target = corrector.should_seek(12000);
        assert!(seek_target.is_some());
    }

    #[test]
    fn test_drift_correction_not_playing() {
        let mut corrector = DriftCorrector::new();
        let mut target = PlaybackSync::new("test".to_string());
        target.position_at_ref_ms = 10000;
        target.ref_wallclock_ms = now_ms();
        target.is_playing = false;

        corrector.update_target(target);

        // Not playing = no correction regardless of position
        let rate = corrector.calculate_rate(50000);
        assert_eq!(rate, 1.0);
    }

    #[test]
    fn test_playback_sync_seek() {
        let mut sync = PlaybackSync::new("test".to_string());
        sync.position_at_ref_ms = 5000;
        sync.is_playing = true;

        sync.seek(20000);
        assert_eq!(sync.position_at_ref_ms, 20000);
    }

    #[test]
    fn test_playback_sync_set_playing_updates_ref() {
        let mut sync = PlaybackSync::new("test".to_string());
        sync.position_at_ref_ms = 5000;
        sync.ref_wallclock_ms = now_ms() - 1000; // 1s ago
        sync.is_playing = true;

        // set_playing(false) should capture current position
        sync.set_playing(false);
        assert!(!sync.is_playing);
        // Position should be approximately 6000 (5000 + 1000ms)
        assert!((5900..=6100).contains(&sync.position_at_ref_ms));
    }

    #[test]
    fn test_current_drift_ms() {
        let mut corrector = DriftCorrector::new();
        let mut target = PlaybackSync::new("test".to_string());
        target.position_at_ref_ms = 10000;
        target.ref_wallclock_ms = now_ms();
        target.is_playing = true;

        corrector.update_target(target);

        let drift = corrector.current_drift_ms(10500);
        assert!(drift.is_some());
        let d = drift.unwrap();
        // Should be approximately +500 (we're 500ms ahead)
        assert!((400..=600).contains(&d));
    }

    // =========================================================================
    // Phase 6: Additional edge case tests
    // =========================================================================

    #[test]
    fn test_clock_sync_no_offset_before_samples() {
        let sync = ClockSync::new();
        assert!(!sync.is_synced());
        assert_eq!(sync.offset_ms(), None);
        assert!(sync.local_to_remote(1000).is_err());
        assert!(sync.remote_to_local(1000).is_err());
    }

    #[test]
    fn test_clock_sync_negative_offset() {
        let mut sync = ClockSync::new();
        // Remote is behind local: remote_time < local_at_remote
        // send=1000, remote=950, recv=1100 -> rtt=100, one_way=50
        // offset = 950 - (1000+50) = -100
        sync.process_pong(1000, 950, 1100);
        let offset = sync.offset_ms().unwrap();
        assert_eq!(offset, -100);

        // Conversions should work with negative offset
        let remote = sync.local_to_remote(1000).unwrap();
        assert_eq!(remote, 900); // 1000 - 100

        let local = sync.remote_to_local(900).unwrap();
        assert_eq!(local, 1000); // 900 + 100
    }

    #[test]
    fn test_clock_sync_max_samples_eviction() {
        let mut sync = ClockSync::new();

        // Add more than max_samples (10) to test eviction
        for i in 0..15u64 {
            sync.process_pong(i * 100, i * 100 + 50, i * 100 + 100);
        }

        assert!(sync.is_synced());
        // After eviction, should still have valid offset
        let offset = sync.offset_ms().unwrap();
        assert_eq!(offset, 0); // All samples have 0 offset
    }

    #[test]
    fn test_clock_sync_zero_rtt() {
        let mut sync = ClockSync::new();
        // Zero RTT (instantaneous round trip)
        sync.process_pong(1000, 1000, 1000);
        assert_eq!(sync.offset_ms(), Some(0));
    }

    #[test]
    fn test_playback_sync_position_with_rate() {
        let mut sync = PlaybackSync::new("test".to_string());
        sync.position_at_ref_ms = 10000;
        sync.ref_wallclock_ms = now_ms() - 1000; // 1 second ago
        sync.is_playing = true;
        sync.playback_rate = 2.0; // Double speed

        // Should be approximately 12000 (10000 + 1000 * 2.0)
        let pos = sync.current_position_ms();
        assert!((11800..=12200).contains(&pos));
    }

    #[test]
    fn test_playback_sync_position_saturates() {
        let mut sync = PlaybackSync::new("test".to_string());
        sync.position_at_ref_ms = u64::MAX - 100;
        sync.ref_wallclock_ms = now_ms() - 1000;
        sync.is_playing = true;
        sync.playback_rate = 1.0;

        // Should saturate at u64::MAX, not wrap around
        let pos = sync.current_position_ms();
        assert_eq!(pos, u64::MAX);
    }

    #[test]
    fn test_playback_sync_update_from() {
        let mut local = PlaybackSync::new("old".to_string());
        local.playback_rate = 0.98; // Local drift correction

        let remote = PlaybackSync {
            content_id: "new_content".to_string(),
            position_at_ref_ms: 5000,
            ref_wallclock_ms: 123456789,
            is_playing: true,
            playback_rate: 1.05,
        };

        local.update_from(&remote);

        assert_eq!(local.content_id, "new_content");
        assert_eq!(local.position_at_ref_ms, 5000);
        assert_eq!(local.ref_wallclock_ms, 123456789);
        assert!(local.is_playing);
        // Playback rate should NOT be copied (it's local drift correction)
        assert_eq!(local.playback_rate, 0.98);
    }

    #[test]
    fn test_drift_corrector_no_target_no_seek() {
        let corrector = DriftCorrector::new();
        assert!(corrector.should_seek(99999).is_none());
        assert!(corrector.current_drift_ms(99999).is_none());
    }

    #[test]
    fn test_drift_correction_with_clock_offset() {
        let mut corrector = DriftCorrector::new();

        // Sync clocks: remote is 100ms ahead
        corrector.clock_sync_mut().process_pong(1000, 1150, 1100);
        // offset = 1150 - (1000+50) = 100

        let mut target = PlaybackSync::new("test".to_string());
        target.position_at_ref_ms = 10000;
        target.ref_wallclock_ms = now_ms() + 100; // Remote time (100ms ahead)
        target.is_playing = true;
        target.playback_rate = 1.0;

        corrector.update_target(target);

        // With clock correction applied, position should be close to target
        let rate = corrector.calculate_rate(10000);
        // Should be within tolerance (1.0) since we account for clock offset
        assert_eq!(rate, 1.0);
    }
}
