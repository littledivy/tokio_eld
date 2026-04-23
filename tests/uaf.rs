//! Reproduces the use-after-free in `EldHistogram::start`.
//!
//! `start()` spawns a tokio task that captures `&mut Histogram<C>` obtained from
//! an `UnsafeCell` inside `self`. Only an `AbortHandle` is stored; `AbortHandle`
//! does not abort on drop. So the task outlives `self`, and its next poll reads
//! and writes freed memory.
//!
//! Run under a sanitizer to observe the UAF deterministically:
//!
//!   RUSTFLAGS="-Z sanitizer=address" \
//!   cargo +nightly test --target x86_64-unknown-linux-gnu --test uaf -- --nocapture
//!
//! Without a sanitizer, the test may appear to pass — heap reuse is undefined.

use std::time::Duration;
use tokio_eld::EldHistogram;

/// Drop the histogram while the sampling task is still scheduled.
/// The task will then access freed memory on its next tick.
#[test]
fn drop_while_task_running_is_uaf() {
  let rt = tokio::runtime::Builder::new_current_thread()
    .enable_time()
    .build()
    .unwrap();

  rt.block_on(async {
    let h = EldHistogram::<u64>::new(1).unwrap();
    h.start();
    // Let the task tick at least once.
    tokio::time::sleep(Duration::from_millis(10)).await;
    // Drop self. The spawned task keeps running and holds &mut into self.ht.
    drop(h);
    // Yield so the task polls again and touches freed memory.
    tokio::time::sleep(Duration::from_millis(20)).await;
  });
}

/// `stop()` aborts but does not join the task. If the task is mid-poll when
/// `self` is dropped, the same UAF window applies.
#[test]
fn stop_then_drop_is_still_racy() {
  let rt = tokio::runtime::Builder::new_current_thread()
    .enable_time()
    .build()
    .unwrap();

  rt.block_on(async {
    let h = EldHistogram::<u64>::new(1).unwrap();
    h.start();
    tokio::time::sleep(Duration::from_millis(10)).await;
    h.stop();
    // abort() is cooperative — the task may already be past the yield_now()
    // point and about to call ht.record() on freed memory.
    drop(h);
    tokio::time::sleep(Duration::from_millis(20)).await;
  });
}
