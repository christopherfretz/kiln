//! The `kiln` and `kiln++` executables (one driver, two names, as clang and
//! clang++). See `spec/driver.md` "Driver modes"; the body is
//! [`kiln_driver::entry::main`], which a wrapper with an in-process back
//! end (spec/driver.md "In-process back end") calls too.
#![forbid(unsafe_code)]

fn main() {
    kiln_driver::entry::main()
}
