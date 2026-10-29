//! The `kiln` executable. See `spec/driver.md`.
#![forbid(unsafe_code)]

/// Stack for the compiling thread. Template instantiation and constant
/// evaluation recurse with the program (clang's default instantiation
/// depth is 1024); the reservation is virtual, pages are touched only
/// as deep as the program goes.
const STACK_BYTES: usize = 1 << 30;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = std::thread::Builder::new()
        .name("kiln".into())
        .stack_size(STACK_BYTES)
        .spawn(move || kiln_driver::run(&args, kiln_driver::Io::std()))
        .ok()
        .and_then(|h| h.join().ok())
        .unwrap_or(70);
    std::process::exit(code);
}
