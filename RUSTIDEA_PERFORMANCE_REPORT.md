# RustIdea performance report

Status: **measurements not collected; performance acceptance is open**.

The test host is an Apple M5 arm64 Mac with macOS 27.2.1. The installed
OpenIDEA 0.1.50 app is available, but neither app was launched for comparative
measurements in this run. Therefore there are no memory, CPU, GPU, startup,
input-latency, file-open, tab-switching or scroll-FPS values to report. No
resource reduction is claimed.

Use `RUSTIDEA_PERFORMANCE_BASELINE.md` for the A–J scenarios, process-tree
measurement rules, five-run minimum and acceptance thresholds. Run the same
fixtures and actions on both products. Record raw samples and medians here,
including any unavailable USS/PSS data. Large-file, long-run and multi-split
results must be recorded separately; do not infer them from unit tests or from
the presence of virtualized rendering code.

Until these measurements exist, the task-book requirements for at least 15%
lower ordinary-project process-tree memory, 60 FPS normal scrolling, no
sustained memory growth and no large-file hangs remain unverified.
