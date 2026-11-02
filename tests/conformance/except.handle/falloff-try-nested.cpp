// [except.handle]: a nested try whose handlers miss the exception propagates to
// the outer dispatch, not to exit (WP-219)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 12:63 warning "non-void function does not return a value in all control paths"
//@ requires: clang<22
// clang 19 spelling; falloff-try-nested-clang22.cpp has clang 22's (WP-377).
int u2(int x) { if consteval { return 1; } }
int u3(int x) { return 1; x++; }
enum E { A, B };
int u4(E e) { switch (e) { case A: return 1; case B: return 2; } x: ; }
int u5(E e) { switch (e) { case A: return 1; case B: break; } }
int u6(E e, int x) { switch (e) { case A: x++; default: return 2; case B: return 1; } }
int u8(E e, int x) { switch (e) { case A: x++; [[fallthrough]]; default: x++; case B: return 1; } }
const E ce = A;
void u7(E e) { switch (e) { case ce: case B: break; } }
int u9(E e) { switch (e) { case A: return 1; case B: return 2; } int y = 0; (void)y; }
