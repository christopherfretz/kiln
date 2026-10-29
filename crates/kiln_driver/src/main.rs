//! The `kiln` executable. See `spec/driver.md`.
#![forbid(unsafe_code)]

use kiln_sema::stack_guard::StackGuard;

/// Compiles on a thread whose stack is `-fkiln-stack-size=` (default
/// 1 GiB, spec/sema-templates.md Q4): template instantiation, parsing and
/// constant evaluation recurse with the program, and the reservation is
/// virtual, so pages are touched only as deep as the program goes. The
/// stack guard installed first on that thread turns exhaustion into a
/// fatal diagnostic (§1 P4, §10.3).
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let size = kiln_driver::templ_opts::stack_size_of(&args);
    let code = std::thread::Builder::new()
        .name("kiln".into())
        .stack_size(size)
        .spawn(move || {
            StackGuard::install(size);
            kiln_driver::run(&args, kiln_driver::Io::std())
        })
        .ok()
        .and_then(|h| h.join().ok())
        .unwrap_or(70);
    std::process::exit(code);
}
