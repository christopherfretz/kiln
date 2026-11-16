// [dcl.asm] GNU inline asm on x86-64: an x87 stack register output
// ("f") cannot also be an input ("+f"); clang 22 rejects the constraint
// ("invalid output constraint '+f' in asm"). kiln accepts it, and with
// -mno-sse2 llc then aborts ("Access past stack top!"). Found running
// GCC's C++ testsuite on x86-64. clang 19 accepts the constraint.
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu
//@ requires: clang>=22
//@ diag: 15:20 error "invalid output constraint '+f' in asm"
//@ xfail(kiln): WP-1473 remainder: x86-64 '+f' asm output constraint is accepted (no x87 read-write check in validate_output_constraint)

double f(double d) {
  for (int i = 0; i < 2; ++i)
    asm volatile(""
                 : "+f"(d));
  return d;
}
