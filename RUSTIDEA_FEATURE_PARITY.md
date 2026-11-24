# Feature parity inventory

This is a directory-level inventory of the upstream snapshot, not proof that
each feature within a directory has been enumerated. All rows require a GPUI UI
and architecture adaptation; reuse is a candidate until its API and tests are
verified. RustIdea has only an early editor, tabs and project tree. The other
features below are not migrated.

| Upstream feature directories | Candidate backend / migration work |
|---|---|
| app, welcome, whats-new, project-trust, settings | Native shell and settings; port frontend state and trust flows |
| editor, editor-lsp, code-insight, lsp-messages | ide-text, ide-language, ide-syntax, ide-lsp, ide-host; native editor and LSP interactions |
| project-tree, search, scratch, todo | ide-vfs, ide-workspace, ide-search; native navigation and frontend logic |
| vcs, github | ide-vcs, ide-github; native Git and diff views |
| terminal, run, run-anything, build, build-tools | ide-terminal, ide-run, ide-build; native terminal and workflows |
| debugger, coverage, profiler, logcat | ide-dap, ide-coverage, ide-profiler, ide-android; native tools |
| dependencies, libraries, dotnet, rails, cucumber, thymeleaf | Existing language/project services where available; port UI logic |
| database, graphql, http-client, web, viewers, notebook | ide-db, ide-http, ide-web, ide-notebook; native replacements for web-based views |
| plugins, mcp, agents, services, tools | ide-plugin, ide-mcp, ide-agent; native extension surfaces and service adapters |
| ssh, remote-dev, deployment | ide-ssh, ide-remote; native remote workflows |
| analysis, color-preview, macros, perf, templates, vim, python-console | Existing services where applicable; audit frontend logic and implement native interactions |

The inventory must be expanded to individual commands and tests before claiming
complete parity. In particular, upstream supports external-file conflict
protection, multi-view synchronization, encoding, LSP, syntax and recovery that
the present editor does not yet provide.
