//! `kiln` and `kiln++` with hod linked in (spec/driver.md "In-process back
//! end"): registers [`kiln_hod::Hod`] as the in-process back end, then
//! runs exactly the stock driver's `main` ([`kiln_driver::entry::main`]:
//! the crash hook, the compile thread, the program name's driver mode).
//! Without `-fkiln-backend=hod` (or `KILN_BACKEND=hod`) it is the stock
//! compiler; `--version` adds one line naming the built-in back end.
//! `KILN_HOD_IN_PROCESS=0` leaves the back end unregistered.
#![forbid(unsafe_code)]

fn main() {
    // `KILN_HOD_IN_PROCESS=0` registers nothing: the wrapper is then the
    // stock driver, with the external route (for comparisons and
    // measurements against the same build).
    if std::env::var_os("KILN_HOD_IN_PROCESS").is_some_and(|v| v == "0") {
        kiln_driver::entry::main()
    }
    if let Err(e) = kiln_hod::register() {
        // Nothing else registers in this binary: unreachable in practice.
        eprintln!("kiln: error: {e}");
        std::process::exit(70);
    }
    kiln_driver::entry::main()
}
