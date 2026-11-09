// -fprofile-instr-generate=<path> (WP-941): clang's InstrProfiling
// lowering turns the path into the hidden, comdat `__llvm_profile_filename`
// constant the profile runtime reads at exit (LLVM_PROFILE_FILE still
// overrides it); kiln emits it with the module. Under -disable-llvm-passes
// neither compiler has it, and the module flag `EnableValueProfiling` is 0.
//@ args: -fprofile-instr-generate=cov/%p-%m.profraw -Xclang -disable-llvm-passes
int f(int x) { return x ? 1 : 2; }
