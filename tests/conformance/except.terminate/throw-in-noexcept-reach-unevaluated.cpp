// [except.terminate], [except.spec]/5: the -Wexceptions reachability walk
// skips unevaluated operands (`sizeof`, `alignof`, `noexcept`,
// `decltype`), and a GNU case range is reached when the switch condition
// is in [lo, hi] (spec/sema.md §7.5).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-unevaluated-expression
//@ diag: 17:37 warning "'f2' has a non-throwing exception specification but can still throw"
//@ diag: 17:6 note "function declared non-throwing here"
//@ diag: 20:53 warning "'f5' has a non-throwing exception specification but can still throw"
//@ diag: 20:5 note "function declared non-throwing here"
//@ diag: 22:48 warning "'f7' has a non-throwing exception specification but can still throw"
//@ diag: 22:5 note "function declared non-throwing here"
//@ diag: 23:67 warning "'f8' has a non-throwing exception specification but can still throw"
//@ diag: 23:5 note "function declared non-throwing here"
void f1() noexcept { (void)sizeof((throw 1, 0)); }
void f2() noexcept { (void)noexcept(throw 1); }
void f3() noexcept { (void)alignof(decltype((throw 1, 0))); }
void f4() noexcept { using T = decltype(throw 1); }
int f5(int x) noexcept { switch (x) { case 1 ... 3: throw 1; } return 0; }
int f6() noexcept { switch (5) { case 1 ... 3: throw 1; case 4 ... 6: return 1; } return 0; }
int f7() noexcept { switch (2) { case 1 ... 3: throw 1; case 4 ... 6: return 1; } return 0; }
int f8() noexcept { switch (9) { case 1 ... 3: return 0; default: throw 1; } }
