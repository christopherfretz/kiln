//! The `kiln` executable. See `spec/driver.md`.
#![forbid(unsafe_code)]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = kiln_driver::run(&args, kiln_driver::Io::std());
    std::process::exit(code);
}
