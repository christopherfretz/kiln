// An internal compiler error while parsing: kiln's banner names the input,
// the phase and the declaration being parsed, then the version and where
// to report it, and the crash reproducer (.ii + .sh) lands in the
// -fcrash-diagnostics-dir the harness passes.
// spec/driver.md "Internal compiler errors", WP-911.
//@ expect: ice
//@ args: -fkiln-crash-for-testing=parse
//@ ice: kiln++: error: internal compiler error: crash requested by -fkiln-crash-for-testing=parse
//@ ice: ice-parse.cpp' (phase: parse, at
//@ ice: ice-parse.cpp:17:1)
//@ ice: note: in compiler source crates/
//@ ice: please submit a bug report to https://github.com/christopherfretz/kiln/issues
//@ ice: note: preprocessed source and associated run script are located at:
//@ ice: ice-parse-
//@ ice: .sh

int first_declaration;
