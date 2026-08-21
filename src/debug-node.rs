// Proxy bin: `cargo run --bin node -- [options] <any node args>` execs
// the real node, forwarding trailing args. PID is preserved so
// RustRover's Cargo debug session lands inside node with the .node
// module loaded.
//
// unix-only: it exists for that debug workflow, which relies on exec.
//
// Options (consumed before node args):
//   --cwd <dir>   working directory for the node process
//
// Resolve node from $PATH (honors fnm/nvm shims) so it works across versions.
use std::os::unix::process::CommandExt;
use std::process::Command;

fn main() -> ! {
    let mut args = std::env::args().skip(1);
    let mut cwd: Option<String> = None;

    let mut node_args: Vec<String> = Vec::new();
    while let Some(a) = args.next() {
        if a == "--cwd" {
            cwd = args.next().or_else(|| {
                eprintln!("node-proxy: --cwd requires a value");
                std::process::exit(1);
            });
        } else {
            node_args.push(a);
        }
    }

    let mut cmd = Command::new("node");
    cmd.args(&node_args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let err = cmd.exec();
    eprintln!("node-proxy: failed to exec node: {err}");
    std::process::exit(1);
}
