# Performance baseline

Status: **not collected**. No memory or speed improvement is claimed. The
installed OpenIDEA 0.1.50 app is available as a candidate baseline; its exact
build, hardware, OS, settings, fixture and process tree must be recorded with
the results. Source snapshot and release binary are not identical evidence.

Candidate measurement host inspected on 2026-11-24: Apple M5, arm64,
25,769,803,776 bytes physical RAM, macOS 27.2.1 build 26B131. The installed
`/Applications/OpenIDEA.app` reports version 0.1.50. No application was
launched and these host facts are not performance measurements.

Run both products on the same machine and fixtures, at least five times per
scenario, recording every sample and the median:

| ID | Scenario | OpenIDEA | RustIdea |
|---|---|---|---|
| A | Idle | Not measured | Not measured |
| B | Open project | Not measured | Not measured |
| C | Open 10 files | Not measured | Not measured |
| D | Open 100,000 lines | Not measured | Not measured |
| E | Open 1,000,000 lines | Not measured | Not measured |
| F | One file in four splits | Not measured | Not implemented |
| G | Continuous input in multiple files | Not measured | Not measured |
| H | Fast scrolling | Not measured | Not measured |
| I | Concurrent LSP tasks | Not measured | Not implemented |
| J | Multiple windows | Not measured | Not implemented |

Capture total process-tree physical memory, macOS footprint, CPU, GPU, cold
startup, file opening, tab switching, input latency and scrolling FPS. Record
unsupported USS/PSS as unavailable rather than zero. Include renderer, backend,
LSP and other child processes. Compare ordinary-project memory against the
required 15% reduction (20% target), check idle CPU and input/scroll regression,
target 60 FPS and check for sustained memory growth. Do not mark acceptance
passed until measured results support it.
