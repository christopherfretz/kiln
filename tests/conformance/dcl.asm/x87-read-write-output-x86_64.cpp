// [dcl.asm] GNU inline asm on x86-64: an x87 stack register output
// ("f") cannot also be an input ("+f"); clang 22 rejects the constraint
// ("invalid output constraint '+f' in asm"), in any alternative and with
// other letters beside it. kiln accepted it, and with -mno-sse2 llc then
// aborted ("Access past stack top!"). Found running GCC's C++ testsuite on
// x86-64. clang 19 accepts a read-write "f" output.
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu
//@ requires: clang>=22
//@ strict-diags
//@ diag: 21:20 error "invalid output constraint '+f' in asm"
//@ diag: 22:21 error "invalid output constraint '+f' in asm"
//@ diag: 23:21 error "invalid output constraint '+&f' in asm"
//@ diag: 24:21 error "invalid output constraint '+rf' in asm"
//@ diag: 25:21 error "invalid output constraint '+r,f' in asm"
//@ diag: 26:21 error "invalid output constraint '=f' in asm"

double f(double d) {
  for (int i = 0; i < 2; ++i)
    asm volatile(""
                 : "+f"(d));
  asm volatile("" : "+f"(d));
  asm volatile("" : "+&f"(d));
  asm volatile("" : "+rf"(d));
  asm volatile("" : "+r,f"(d));
  asm volatile("" : "=f"(d));
  return d;
}
