// Copyright 2024 Divy Srivastava <dj.srivastava23@gmail.com>

//! _tokio_eld_ provides a histogram-based sampler for recording and analyzing
//! event loop delays in a current_thread Tokio runtime. The API is similar to
//! Node.js's `perf_hooks.monitorEventLoopDelay()`.
//!
//! # EldHistogram
//!
//! EldHistogram supports recording and analyzing event loop delay using a High Dynamic Range (HDR)
//! Histogram. The recorded delays are in nanoseconds.
//!
//! Refer to documentation for [`hdrhistogram::Histogram`](https://docs.rs/hdrhistogram/latest/hdrhistogram/struct.Histogram.html) for more information
//! on how to use the core data structure.
//!
//! # Usage
//!
//! ```rust
//! use tokio_eld::EldHistogram;
//!
//! # #[tokio::test]
//! # async fn test_example() {
//! let h = EldHistogram::<u64>::new(20).unwrap();
//! h.start();
//! // do some work
//! h.stop();
//!
//! println!("min: {}", h.min());
//! println!("max: {}", h.max());
//! println!("mean: {}", h.mean());
//! println!("stddev: {}", h.stdev());
//! println!("p50: {}", h.value_at_percentile(50.0));
//! println!("p90: {}", h.value_at_percentile(90.0));
//! # }
//! ```

use hdrhistogram::errors::CreationError;
use hdrhistogram::Counter;
use hdrhistogram::Histogram;

use tokio::task::AbortHandle;

use std::cell::Cell;
use std::sync::Arc;
use std::sync::Mutex;

/// Error types used in this crate.
#[derive(Debug)]
pub enum Error {
  CreationError(CreationError),
}

impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Error::CreationError(e) => write!(f, "CreationError: {}", e),
    }
  }
}

impl std::error::Error for Error {}

impl From<CreationError> for Error {
  fn from(e: CreationError) -> Self {
    Error::CreationError(e)
  }
}

pub type Result<T> = std::result::Result<T, Error>;

/// A `Histogram` that can be written to concurrently by multiple tasks, used
/// to measure event loop delays.
///
/// The underlying `hdrhistogram::Histogram` is owned through `Arc<Mutex<_>>`
/// so that the sampling task can safely hold its own reference; the histogram
/// lives until both the task and the `EldHistogram` handle are dropped.
pub struct EldHistogram<C: Counter> {
  ht: Arc<Mutex<Histogram<C>>>,

  fut: Cell<Option<AbortHandle>>,

  resolution: usize,
}

impl<C: Counter + Send + 'static> EldHistogram<C> {
  /// Creates a new `EldHistogram` with the given timer resolution that samples
  /// the event loop delay over time. The delays are recorded in nanoseconds.
  pub fn new(resolution: usize) -> Result<Self> {
    let ht = Histogram::<C>::new(5)?;

    Ok(Self {
      ht: Arc::new(Mutex::new(ht)),
      fut: Cell::new(None),
      resolution,
    })
  }

  /// Start the update interval recorder.
  ///
  /// This will start a new task that will record the event loop delay at the
  /// given resolution.
  pub fn start(&self) {
    let r = self.resolution as u64;
    let ht = Arc::clone(&self.ht);

    let fut = tokio::spawn(async move {
      let mut interval =
        tokio::time::interval(tokio::time::Duration::from_millis(r));
      loop {
        interval.tick().await;

        let clock = tokio::time::Instant::now();

        tokio::task::yield_now().await;
        if let Ok(mut ht) = ht.lock() {
          let _ = ht.record(clock.elapsed().as_nanos() as u64);
        }
      }
    });

    if let Some(prev) = self.fut.replace(Some(fut.abort_handle())) {
      prev.abort();
    }
  }

  /// Stop the update interval recorder.
  pub fn stop(&self) {
    if let Some(fut) = self.fut.take() {
      fut.abort();
    }
  }

  fn with_ht<R>(&self, f: impl FnOnce(&Histogram<C>) -> R) -> R {
    let g = self.ht.lock().expect("histogram mutex poisoned");
    f(&g)
  }

  fn with_ht_mut<R>(&self, f: impl FnOnce(&mut Histogram<C>) -> R) -> R {
    let mut g = self.ht.lock().expect("histogram mutex poisoned");
    f(&mut g)
  }

  /// Reset the histogram, clearing all recorded values.
  pub fn reset(&self) {
    self.with_ht_mut(|h| h.reset());
  }

  /// Record a raw sample into the histogram.
  pub fn record(&self, value: u64) {
    let _ = self.with_ht_mut(|h| h.record(value));
  }

  /// The number of samples recorded by the histogram.
  pub fn len(&self) -> u64 {
    self.with_ht(|h| h.len())
  }

  /// `true` if no samples have been recorded.
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// The minimum recorded sample.
  pub fn min(&self) -> u64 {
    self.with_ht(|h| h.min())
  }

  /// The maximum recorded sample.
  pub fn max(&self) -> u64 {
    self.with_ht(|h| h.max())
  }

  /// The arithmetic mean of recorded samples.
  pub fn mean(&self) -> f64 {
    self.with_ht(|h| h.mean())
  }

  /// The standard deviation of recorded samples.
  pub fn stdev(&self) -> f64 {
    self.with_ht(|h| h.stdev())
  }

  /// The value at the given percentile (`percentile` ∈ (0, 100]).
  pub fn value_at_percentile(&self, percentile: f64) -> u64 {
    self.with_ht(|h| h.value_at_percentile(percentile))
  }
}

impl<C: Counter> Drop for EldHistogram<C> {
  fn drop(&mut self) {
    if let Some(fut) = self.fut.take() {
      fut.abort();
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_eld() {
    let h = EldHistogram::<u64>::new(20).unwrap();
    h.start();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    h.stop();

    assert!(h.min() > 0);
  }
}
