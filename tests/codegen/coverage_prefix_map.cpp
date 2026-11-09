// spec/codegen.md §5.4 "Instrumentation", spec/driver.md "Coverage flags"
// (WP-1003): the mapping's filenames are clang's normalized names. The
// compilation directory is the first filename; `./` and `x/../` are
// removed and the last matching -fcoverage-prefix-map applies.
//@ args: -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/a/./x/../b -fcoverage-prefix-map=/a=/first -fcoverage-prefix-map=/a=/z -Xclang -disable-llvm-passes
int f(int x) { return x ? 1 : 2; }
