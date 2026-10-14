# Dynamic service prototype

`SrvDynLoader` loads a native library with `libloading`. The exported functions
for `S1` and `Add` are written out in [`examples/flatten-so`](../examples/flatten-so/src/lib.rs)
as the intended shape of a future `export!` macro. No export macro is implemented yet.

```sh
cargo build -p serviceless-flatten-so
cargo run -p serviceless-dynamic --example load_flatten -- target/debug
```

The example builds a `cdylib`, resolves its file name with
`libloading::library_filename`, loads `S1`, calls `Add`, sends `Add`, and closes
the service. On Windows, macOS, and Linux the same example selects the native
library extension automatically. `load_lib` accepts either a full library path
or a directory containing a library named for the service.

The host and plugin must agree on the exact C-compatible, `Copy` message and
result layouts. `DynamicMessage` is an unsafe trait because Rust cannot verify
this agreement. The plugin owns the service instance and runtime. A successful
`call` takes ownership of the callback context and must invoke exactly one
callback. A nonzero immediate return must invoke no callback. The result
pointer is valid only during the success callback.

`close().await` rejects new calls and waits for accepted callbacks before
destroying the instance and unloading the library. Dropping a service with
pending callbacks deliberately retains the library to avoid unloading code
while a callback runs; applications should call `close()`.

The prototype supports an isolated Tokio runtime in the plugin. The shared
runtime configuration is reserved: a separately linked `cdylib` does not share
Tokio's thread-local `Handle::current()` with the host, so the example rejects
that configuration until a cross-library scheduler protocol is defined.
