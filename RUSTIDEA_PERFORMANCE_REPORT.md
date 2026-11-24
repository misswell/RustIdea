# RustIdea performance report

Status: **document-engine baseline collected; UI/comparative acceptance is open**.

The test host is an Apple M5 arm64 Mac with macOS 27.2.1. The engine benchmark
was run in the optimized `bench` profile with Criterion, 10 samples and a 3-second
measurement window per case:

| Document | Single-line edit (median) | Undo + redo (median) | 60-line viewport reads (median) |
|---|---:|---:|---:|
| 100,000 lines | 1.326 µs | 3.167 µs | 5.946 µs |
| 1,000,000 lines | 1.431 µs | 3.853 µs | 6.860 µs |

These values cover `ide-text`/Ropey document mutation, bounded undo history
lookup and visible-line extraction. They do not measure `shape_line`, GPUI
paint, clipboard, IME, file watching, LSP or memory. The editor's bounded
layout cache and 4,000-line/512-KiB syntax cutoff are code-level safeguards for
those paths, not substituted performance evidence.

No OpenIDEA/RustIdea process-tree comparison exists yet. There are still no
memory, CPU, GPU, startup, input-latency, file-open, tab-switching or scroll-FPS
measurements. No resource reduction is claimed.

Use `RUSTIDEA_PERFORMANCE_BASELINE.md` for the A–J scenarios, process-tree
measurement rules, five-run minimum and acceptance thresholds. Run the same
fixtures and actions on both products. Record raw samples and medians here,
including any unavailable USS/PSS data. Large-file, long-run and multi-split
results must be recorded separately; do not infer them from unit tests or from
the presence of virtualized rendering code.

Until these measurements exist, the task-book requirements for at least 15%
lower ordinary-project process-tree memory, 60 FPS normal scrolling, no
sustained memory growth and no large-file hangs remain unverified.
