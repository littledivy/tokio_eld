// Copyright 2024 Divy Srivastava <dj.srivastava23@gmail.com>

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
//! let mut h = EldHistogram::<u64>::new(20).unwrap();
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

use std::cell::UnsafeCell;

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

/// A `Histogram` that can written to concurrently by mutiple tasks, used to
/// measure event loop delays.
///
/// Look at
/// [`hdrhistogram::Histogram`](https://docs.rs/hdrhistogram/latest/hdrhistogram/struct.Histogram.html) for more information on how to use the
/// core data structure.
#[derive(Debug)]
pub struct EldHistogram<C: Counter> {
  ht: UnsafeCell<Histogram<C>>,

  fut: Option<AbortHandle>,

  resolution: usize,
}

impl<C: Counter> std::ops::Deref for EldHistogram<C> {
  type Target = Histogram<C>;

  fn deref(&self) -> &Self::Target {
    unsafe { &*self.ht.get() }
  }
}

impl<C: Counter> std::ops::DerefMut for EldHistogram<C> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe { &mut *self.ht.get() }
  }
}

impl<C: Counter + Send + 'static> EldHistogram<C> {
  /// Creates a new `EldHistogram` with the given timer resolution that samples
  /// the event loop delay over time. The delays are recorded in nanoseconds.
  pub fn new(resolution: usize) -> Result<Self> {
    let ht = Histogram::<C>::new(5)?;

    Ok(Self {
      ht: UnsafeCell::new(ht),
      fut: None,
      resolution,
    })
  }

  /// Start the update interval recorder.
  ///
  /// This will start a new task that will record the event loop delay at the
  /// given resolution.
  pub fn start(&mut self) {
    let r = self.resolution as u64;

    let ht = unsafe { &mut *self.ht.get() };
    let fut = tokio::spawn(async move {
      let mut interval =
        tokio::time::interval(tokio::time::Duration::from_millis(r));
      loop {
        interval.tick().await;

        let clock = tokio::time::Instant::now();

        tokio::task::yield_now().await;
        let _ = ht.record(clock.elapsed().as_nanos() as u64);
      }
    });

    self.fut = Some(fut.abort_handle());
  }

  /// Stop the update interval recorder.
  pub fn stop(&mut self) {
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
    let mut h = EldHistogram::<u64>::new(20).unwrap();
    h.start();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    h.stop();

    assert!(h.min() > 0);
  }
}
