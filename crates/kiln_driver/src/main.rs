//! The `kiln` and `kiln++` executables (one driver, two names, as clang and
//! clang++). See `spec/driver.md` "Driver modes".
#![forbid(unsafe_code)]

use kiln_sema::stack_guard::StackGuard;

/// Compiles on a thread whose stack is `-fkiln-stack-size=` (default
/// 1 GiB, spec/sema-templates.md Q4): template instantiation, parsing and
/// constant evaluation recurse with the program, and the reservation is
/// virtual, so pages are touched only as deep as the program goes. The
/// stack guard installed first on that thread turns exhaustion into a
/// fatal diagnostic (§1 P4, §10.3). The program name (`argv[0]`) picks the
/// driver mode.
fn main() {
    let mut command_line = std::env::args();
    let argv0 = command_line.next().unwrap_or_else(|| "kiln".to_owned());
    let args: Vec<String> = command_line.collect();
    let size = kiln_driver::templ_opts::stack_size_of(&args);
    let code = std::thread::Builder::new()
        .name("kiln".into())
        .stack_size(size)
        .spawn(move || {
            StackGuard::install(size);
            kiln_driver::run_as(&argv0, &args, kiln_driver::Io::std())
        })
        .ok()
        .and_then(|h| h.join().ok())
        .unwrap_or(70);
    std::process::exit(code);
}
