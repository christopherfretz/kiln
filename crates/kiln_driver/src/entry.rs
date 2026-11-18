//! The `kiln` and `kiln++` executables' `main` (one driver, two names, as
//! clang and clang++; spec/driver.md "Driver modes"), as a library
//! function so that a wrapper binary that registers an in-process back
//! end first (spec/driver.md "In-process back end": `kiln_hod`'s
//! `kiln++`) runs exactly what the stock binary runs.
use kiln_sema::stack_guard::StackGuard;

/// Installs the internal-compiler-error hook first (spec/driver.md
/// "Internal compiler errors"), then runs the driver on the process's
/// command line and exits with its status; a panic anywhere unwinds back
/// here and becomes one report and its exit code.
pub fn main() -> ! {
    crate::process_started();
    crate::crash::install_hook();
    let mut command_line = std::env::args();
    let argv0 = command_line.next().unwrap_or_else(|| "kiln".to_owned());
    let raw: Vec<String> = command_line.collect();
    let code = match std::panic::catch_unwind(|| compile(&argv0, &raw)) {
        Ok(code) => code,
        Err(_) => crate::crash::report(&argv0, &raw),
    };
    crate::phase_report();
    std::process::exit(code);
}

/// Compiles on a thread whose stack is `-fkiln-stack-size=` (default
/// 1 GiB, spec/sema-templates.md Q4): template instantiation, parsing and
/// constant evaluation recurse with the program, and the reservation is
/// virtual, so pages are touched only as deep as the program goes. The
/// stack guard installed first on that thread turns exhaustion into a
/// fatal diagnostic (§1 P4, §10.3). The program name (`argv[0]`) picks the
/// driver mode. A panic on the compile thread is re-raised here.
fn compile(argv0: &str, raw: &[String]) -> i32 {
    let args = crate::rsp::expand(raw);
    let size = crate::templ_opts::stack_size_of(&args);
    let prog = std::path::Path::new(argv0)
        .file_name()
        .map_or_else(|| "kiln".to_owned(), |n| n.to_string_lossy().into_owned());
    let argv0 = argv0.to_owned();
    let spawned =
        std::thread::Builder::new().name("kiln".into()).stack_size(size).spawn(move || {
            StackGuard::install(size);
            crate::run_as(&argv0, &args, crate::Io::std())
        });
    match spawned {
        Ok(h) => h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)),
        // The stack is reserved up front, so a low address-space limit
        // (`ulimit -v`) fails here, before anything is compiled: say so
        // rather than exit silently (spec/driver.md "Compile thread").
        Err(e) => {
            eprintln!(
                "{prog}: error: cannot create the compile thread with a {size}-byte stack: {e} \
                 (lower it with -fkiln-stack-size=)"
            );
            70
        }
    }
}
