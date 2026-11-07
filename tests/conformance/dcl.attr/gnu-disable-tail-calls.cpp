// GNU `disable_tail_calls` / `clang::disable_tail_calls` (WP-682; clang
// `DisableTailCallsAttr`, `MutualExclusions<[Naked, DisableTailCalls]>`):
// it takes no arguments and applies to functions only (a warning, and
// ignored, elsewhere); with `naked` on the same function, the later of the
// two is an error at it with a note at the earlier, on one declaration or
// across redeclarations. `__has_attribute` reports it.
// [dcl.attr.grammar]/5 (implementation-defined attributes).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:16 warning "'disable_tail_calls' attribute only applies to functions and Objective-C methods"
//@ diag: 23:16 error "'naked' and 'disable_tail_calls' attributes are not compatible"
//@ diag: 22:16 note "conflicting attribute is here"
//@ diag: 24:23 error "'disable_tail_calls' and 'naked' attributes are not compatible"
//@ diag: 24:16 note "conflicting attribute is here"
//@ diag: 25:36 error "'naked' and 'disable_tail_calls' attributes are not compatible"
//@ diag: 25:16 note "conflicting attribute is here"
//@ diag: 26:16 error "'disable_tail_calls' attribute takes no arguments"
#if !__has_attribute(disable_tail_calls) || !__has_cpp_attribute(clang::disable_tail_calls)
#error "disable_tail_calls not reported"
#endif
__attribute__((disable_tail_calls)) int v;
__attribute__((disable_tail_calls)) void n1();
__attribute__((naked)) void n1() {}
__attribute__((naked, disable_tail_calls)) void n2() {}
__attribute__((disable_tail_calls, naked)) void n3() {}
__attribute__((disable_tail_calls(1))) void q();
[[clang::disable_tail_calls]] int ok(int);
struct S {
  __attribute__((disable_tail_calls)) virtual int m();
};
