use hdrhistogram::{
  sync::{IdleRecorder, Recorder, SyncHistogram},
  Histogram,
};
use tokio::task::JoinHandle;

#[derive(Debug)]
pub enum Error {}

impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "Error")
  }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct EldHistogram {
  sync: SyncHistogram<u64>,

  fut: Option<JoinHandle<()>>,

  resolution: usize,
}

impl std::ops::Deref for EldHistogram {
  type Target = SyncHistogram<u64>;

  fn deref(&self) -> &Self::Target {
    &self.sync
  }
}

impl std::ops::DerefMut for EldHistogram {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.sync
  }
}

impl EldHistogram {
  pub fn new(resolution: usize) -> Result<Self> {
    let ht = Histogram::<u64>::new(5).unwrap();

    let sync = ht.into_sync();

    Ok(Self {
      sync,
      fut: None,
      resolution,
    })
  }

  pub fn start(&mut self) {
    let recorder = self.sync.recorder().into_idle();
    let r = self.resolution as u64;

    let fut = tokio::spawn(async move {
      let mut interval =
        tokio::time::interval(tokio::time::Duration::from_millis(r));
      loop {
        interval.tick().await;

        let mut recorder = recorder.recorder();
        let clock = tokio::time::Instant::now();

        tokio::task::yield_now().await;
        recorder.record(clock.elapsed().as_millis() as u64).unwrap();
      }
    });

    self.fut = Some(fut);
  }

  pub fn stop(&mut self) {
    if let Some(fut) = self.fut.take() {
      fut.abort();
    }

    self.refresh();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_eld() {
    let mut h = EldHistogram::new(10).unwrap();
    h.start();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    h.stop();

    assert!(h.min() <= 5);
  }
}
