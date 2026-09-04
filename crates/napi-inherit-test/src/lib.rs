//! End-to-end verification of `#[layer]`: a three-layer chain
//! `InheritBase -> InheritMid -> InheritLeaf`. The macro generates the
//! `LayerMembers` + `LayerBuild` impls (the bridge dispatches the typed
//! constructor params and calls the user's pure-data `#[layer(constructor)]`
//! method) and mounts each class onto `module.exports` via
//! `register_module_export`, so a `napi build` produces both `index.d.ts`
//! (extends-typed, constructor signature concatenated from the chain) and
//! `index.cjs` with `module.exports.InheritBase = nativeBinding.InheritBase`
//! etc.

#[macro_use]
extern crate napi_derive;

mod generator_chain;
mod manual_chain;
mod proc_chain;

// The ava worker's stderr/stdout are socketpairs that node attaches at
// startup via `pipe.open`, which sets O_NONBLOCK. With concurrent output
// forwarding the buffer can fill, and a Rust `eprintln!` write then fails
// with EAGAIN - the panic that crosses the napi FFI boundary aborts the
// process (SIGABRT). Keep the descriptors blocking so a full buffer waits
// for the host to consume it instead.
#[module_init]
fn restore_blocking_stdio() {
    for fd in [1, 2] {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags != -1 {
            unsafe { libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) };
        }
    }
}
