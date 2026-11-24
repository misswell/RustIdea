# RustIdea vendor note

This is an unmodified copy of `xattr` 0.2.3 except for the Linux/Android
`ENOATTR` cfg in `src/sys/mod.rs`, which maps the Linux errno constant to
`ENODATA`. Linux's libc does not define `ENOATTR`; the upstream crate's
Mac/BSD path already uses the equivalent `ENOATTR` constant.

Upstream: <https://github.com/Stebalien/xattr/tree/v0.2.3>

The crate is MIT OR Apache-2.0. Both license texts are retained here. The patch
is required for Linux builds reached through `zed-async-tar`.
