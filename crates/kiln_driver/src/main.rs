//! The `kiln` driver. See `docs/ARCHITECTURE.md` ("Driver") and `spec/driver.md`.
#![forbid(unsafe_code)]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        println!("kiln version {} (C++26 frontend, LLVM IR backend)", env!("CARGO_PKG_VERSION"));
        return;
    }
    eprintln!("kiln: error: no input files (driver not implemented yet; see docs/ROADMAP.md)");
    std::process::exit(1);
}
