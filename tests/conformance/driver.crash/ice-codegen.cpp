// An internal compiler error while generating LLVM IR (`-c`).
// spec/driver.md "Internal compiler errors", WP-911.
//@ expect: ice
//@ args: -c -fkiln-crash-for-testing=codegen
//@ ice: internal compiler error: crash requested by -fkiln-crash-for-testing=codegen
//@ ice: ice-codegen.cpp' (phase: codegen)
//@ ice: .ii
//@ ice: .sh

int f() { return 1; }
