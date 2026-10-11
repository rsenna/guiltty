//! Linux-only E2E test: drives a real kitty binary under Xvfb to verify that
//! `KittyBackend`'s escape sequences are accepted (`OK`) by the reference terminal.
//!
//! `#[ignore]`'d by default: it needs Xvfb and a kitty binary (see
//! `scripts/fetch-kitty.sh`), so it is not part of the fast unit-test loop. Run on
//! Linux with `cargo test --workspace -- --ignored`.
//!
//! macOS has no Xvfb equivalent (Quartz, not X11), so this automated tier is
//! Linux-only; macOS verification stays manual (see `docs/manual-kitty-verification.md`).

use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// Owns child processes and kills them unconditionally on drop, so a panic or early
/// return in the orchestration test can't leak Xvfb/kitty processes, the temp control
/// socket, or the temp result file.
struct ProcessGuard {
    children: Vec<Child>,
}

impl ProcessGuard {
    fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }

    fn push(&mut self, child: Child) {
        self.children.push(child);
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Returns true if no process with `pid` is running, checked via `kill -0` (a standard
/// Unix utility, so no extra dependency beyond `std::process::Command`).
fn process_is_dead(pid: u32) -> bool {
    match Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) => !status.success(),
        Err(_) => true,
    }
}

#[test]
fn guard_kills_children_on_drop() {
    let mut guard = ProcessGuard::new();
    let child = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("spawn sleep");
    let pid = child.id();
    guard.push(child);
    drop(guard);
    assert!(
        process_is_dead(pid),
        "child {pid} should be dead after drop"
    );
}

#[test]
fn guard_kills_children_on_panic() {
    let pid = std::sync::atomic::AtomicU32::new(0);
    let result = std::panic::catch_unwind(|| {
        let mut guard = ProcessGuard::new();
        let child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn sleep");
        pid.store(child.id(), std::sync::atomic::Ordering::SeqCst);
        guard.push(child);
        panic!("simulated panic");
    });
    assert!(result.is_err(), "the closure should have panicked");
    let pid = pid.load(std::sync::atomic::Ordering::SeqCst);
    assert!(pid != 0, "pid should have been set");
    assert!(
        process_is_dead(pid),
        "child {pid} should be dead after panic"
    );
}

#[test]
#[ignore = "requires Xvfb and a kitty binary (Linux only)"]
fn e2e_kitty_accepts_transmission() {
    if !cfg!(target_os = "linux") {
        eprintln!("skipping: Xvfb-backed kitty E2E is Linux-only");
        return;
    }

    let mut guard = ProcessGuard::new();

    // 1. Start Xvfb on a dynamically allocated display number (never a fixed :99-style
    //    number, so concurrent runs or a dev's own X server don't collide).
    let display = find_free_display();
    let xvfb = Command::new("Xvfb")
        .arg(format!(":{display}"))
        .arg("-screen")
        .arg("0")
        .arg("1024x768x24")
        .spawn()
        .expect("spawn Xvfb");
    guard.push(xvfb);

    // 2. Start kitty with software rendering (so its GPU path initializes against
    //    Xvfb's virtual framebuffer) and a per-run remote-control socket.
    let socket =
        std::env::temp_dir().join(format!("guiltty-kitty-e2e-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket);
    let kitty = Command::new("kitty")
        .env("DISPLAY", format!(":{display}"))
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .arg("--listen-on")
        .arg(format!("unix:{}", socket.display()))
        .arg("--override")
        .arg("allow_remote_control=socket-only")
        .spawn()
        .expect("spawn kitty");
    guard.push(kitty);

    // 3. Wait for kitty's control socket to appear (it is created once kitty is up).
    let deadline = Instant::now() + Duration::from_secs(15);
    while !socket.exists() {
        if Instant::now() > deadline {
            panic!("timed out waiting for kitty's control socket");
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // 4. Launch the harness inside a kitty window via remote control. The harness talks
    //    graphics protocol over its own stdout/stdin (that window's PTY), not the socket.
    let result_file =
        std::env::temp_dir().join(format!("guiltty-kitty-e2e-{}.result", std::process::id()));
    let _ = std::fs::remove_file(&result_file);
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/kitty_e2e_harness");
    let launch = Command::new("kitty")
        .arg("@")
        .arg("--to")
        .arg(format!("unix:{}", socket.display()))
        .arg("launch")
        .arg(&harness)
        .arg(&result_file)
        .status()
        .expect("launch harness via kitty remote control");
    assert!(launch.success(), "kitty @ launch should succeed");

    // 5. Poll the result file against a hard deadline (kitty's response could in
    //    principle never arrive, so fail rather than hang).
    let deadline = Instant::now() + Duration::from_secs(30);
    let result = loop {
        if let Ok(content) = std::fs::read_to_string(&result_file) {
            break content;
        }
        if Instant::now() > deadline {
            panic!("timed out waiting for the harness result file");
        }
        std::thread::sleep(Duration::from_millis(100));
    };

    assert_eq!(
        result.trim(),
        "PASS",
        "kitty should accept the transmission"
    );
}

/// Probes for a free X11 display number by trying to bind the X socket for each
/// candidate, returning the first that is not already in use.
fn find_free_display() -> u32 {
    for display in 100..200 {
        let path = format!("/tmp/.X11-unix/X{display}");
        if !std::path::Path::new(&path).exists() {
            return display;
        }
    }
    panic!("no free X11 display number found in 100..200");
}
