// [driver] clang 19's ACLE operand checks (`SemaARM`): an exclusive access
// whose address drops a qualifier of its pointee when converted to the
// volatile (const, for a load) pointee warns; a system-register operand is
// a string literal holding a name or five in-range `o0:op1:CRn:CRm:op2`
// fields; a write of a PSTATE field takes an immediate in its range; the
// `prefetch` and barrier immediates are constants in range
// (spec/codegen.md "Target builtins"). WP-846.
//@ requires: target-aarch64
//@ args: -march=armv9.4-a+d128
//@ verify
template <int N> void g() {
  __builtin_arm_dmb(N); // expected-error {{argument value 16 is outside the valid range [0, 15]}}
  __builtin_arm_wsr("pan", N); // expected-error {{argument value 16 is outside the valid range [0, 15]}}
}
template void g<16>(); // expected-note {{in instantiation of function template specialization 'g<16>' requested here}}

void f(const int *ci, int *__restrict *rp, const volatile long *cvl, int x, const char *s) {
  __builtin_arm_strex(1, ci); // expected-warning {{passing 'const int *' to parameter of type 'volatile int *' discards qualifiers}}
  __builtin_arm_ldrex(rp); // expected-warning {{passing 'int *__restrict *' to parameter of type 'int *const volatile *' discards qualifiers}}
  __builtin_arm_strex(1, cvl); // expected-warning {{passing 'const volatile long *' to parameter of type 'volatile long *' discards qualifiers}}
  (void)__builtin_arm_ldrex(cvl);
  (void)__builtin_arm_rsr("bogus_reg");
  (void)__builtin_arm_rsr64("1:7:15:15:7");
  __builtin_arm_rsr("1:2:3:4"); // expected-error {{invalid special register for builtin}}
  __builtin_arm_rsr("4:2:3:4:5"); // expected-error {{invalid special register for builtin}}
  __builtin_arm_rsrp("1:8:3:4:5"); // expected-error {{invalid special register for builtin}}
  __builtin_arm_rsr128("1:7:16:15:7"); // expected-error {{invalid special register for builtin}}
  __builtin_arm_rsr("a:b:c:d:e"); // expected-error {{invalid special register for builtin}}
  __builtin_arm_rsr("1:2:3::5"); // expected-error {{invalid special register for builtin}}
  __builtin_arm_rsr64(s); // expected-error {{expression is not a string literal}}
  __builtin_arm_wsr("daifset", 16); // expected-error {{argument value 16 is outside the valid range [0, 15]}}
  __builtin_arm_wsr("PM", 2); // expected-error {{argument value 2 is outside the valid range [0, 1]}}
  __builtin_arm_wsrp("daifset", 0); // expected-error {{argument to '__builtin_arm_wsrp' must be a constant integer}}
  __builtin_arm_wsr("SPSel", 1);
  __builtin_arm_wsr64("1:2:3:4:5", 99);
  __builtin_arm_wsr128("pan", 99);
  __builtin_arm_dmb(16); // expected-error {{argument value 16 is outside the valid range [0, 15]}}
  __builtin_arm_dsb(x); // expected-error {{argument to '__builtin_arm_dsb' must be a constant integer}}
  __builtin_arm_isb(15);
  __builtin_arm_prefetch(ci, 2, 0, 0, 1); // expected-error {{argument value 2 is outside the valid range [0, 1]}}
  __builtin_arm_prefetch(ci, 0, 4, 0, 1); // expected-error {{argument value 4 is outside the valid range [0, 3]}}
  __builtin_arm_prefetch(ci, 0, 0, 2, 1); // expected-error {{argument value 2 is outside the valid range [0, 1]}}
  __builtin_arm_prefetch(ci, 0, 0, 0, 2); // expected-error {{argument value 2 is outside the valid range [0, 1]}}
  __builtin_arm_prefetch(ci, x, 0, 0, 0); // expected-error {{argument to '__builtin_arm_prefetch' must be a constant integer}}
  __builtin_arm_prefetch(ci, 1, 3, 1, 1);
}
