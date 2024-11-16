[![Crates.io](https://img.shields.io/crates/v/tokio_eld.svg)](https://crates.io/crates/tokio_eld)

[Documentation](https://docs.rs/tokio_eld)

_tokio_eld_ provides a histogram-based sampler for recording and analyzing 
event loop delays in a current_thread Tokio runtime. The API is similar to
Node.js's `perf_hooks.monitorEventLoopDelay()`.

```rust
use tokio_eld::EldHistogram;

let mut histogram = EldHistogram::<u64>::new(20)?; // 20 ms resolution

h.start();
// do some work
h.stop();

println!("min: {}", h.min());
println!("max: {}", h.max());
println!("mean: {}", h.mean());
println!("stddev: {}", h.stdev());
println!("p50: {}", h.value_at_percentile(50.0));
println!("p90: {}", h.value_at_percentile(90.0));
```
