// [except.terminate], [except.spec]/5: clang's -Wexceptions check for a
// throw escaping a non-throwing function counts only the throws
// reachable in the body's CFG: none after return/break/goto/noreturn
// calls, none behind a constant condition, `&&`/`||` decided by one
// side, a constant switch, a fully covered enumeration switch, handlers
// reached only by a throw in their try-block, and nothing at all in a
// function with an `if consteval` (spec/sema.md §7.5).
//@ expect: compile-ok
//@ args: -Wno-unused-value
//@ strict-diags
//@ diag: 48:42 warning "'f5' has a non-throwing exception specification but can still throw"
//@ diag: 48:6 note "function declared non-throwing here"
//@ diag: 53:23 warning "'f10' has a non-throwing exception specification but can still throw"
//@ diag: 53:6 note "function declared non-throwing here"
//@ diag: 55:54 warning "'f12' has a non-throwing exception specification but can still throw"
//@ diag: 55:6 note "function declared non-throwing here"
//@ diag: 58:63 warning "'f15' has a non-throwing exception specification but can still throw"
//@ diag: 58:6 note "function declared non-throwing here"
//@ diag: 59:34 warning "'f16' has a non-throwing exception specification but can still throw"
//@ diag: 59:6 note "function declared non-throwing here"
//@ diag: 61:34 warning "'f18' has a non-throwing exception specification but can still throw"
//@ diag: 61:6 note "function declared non-throwing here"
//@ diag: 62:78 warning "'f19' has a non-throwing exception specification but can still throw"
//@ diag: 62:6 note "function declared non-throwing here"
//@ diag: 65:55 warning "'f22' has a non-throwing exception specification but can still throw"
//@ diag: 65:6 note "function declared non-throwing here"
//@ diag: 66:61 warning "'f23' has a non-throwing exception specification but can still throw"
//@ diag: 66:6 note "function declared non-throwing here"
//@ diag: 69:32 warning "'f26' has a non-throwing exception specification but can still throw"
//@ diag: 69:6 note "function declared non-throwing here"
//@ diag: 72:48 warning "'f29' has a non-throwing exception specification but can still throw"
//@ diag: 72:6 note "function declared non-throwing here"
//@ diag: 73:26 warning "'f30' has a non-throwing exception specification but can still throw"
//@ diag: 73:6 note "function declared non-throwing here"
//@ diag: 75:92 warning "'f32' has a non-throwing exception specification but can still throw"
//@ diag: 75:6 note "function declared non-throwing here"
//@ requires: clang<22
// clang 19 spelling; throw-in-noexcept-reach-clang22.cpp has clang 22's (WP-394).
[[noreturn]] void die();
struct S { [[noreturn]] void die(); };
void g();
enum E { A, B };
constexpr bool k = false;
void f1() noexcept { return; throw 1; }
void f2() noexcept { if (false) throw 2; }
void f3() noexcept { while (true) {} throw 3; }
void f4(S s) noexcept { die(); s.die(); throw 4; }
void f5() noexcept { for (;;) { break; } throw 5; }
void f6(int x) noexcept { switch (x) { case 1: return; default: return; } throw 6; }
void f7() noexcept { goto L; throw 7; L:; }
void f8() noexcept { sizeof(int) ? (void)0 : throw 8; }
void f9() noexcept { do { return; } while (0); throw 9; }
void f10() noexcept { throw 10; throw 11; }
void f11() noexcept { try { g(); } catch (...) { throw 12; } }
void f12() noexcept { try { throw 1; } catch (int) { throw 13; } }
void f13(E e) noexcept { switch (e) { case A: return; case B: return; } throw 14; }
void f14() noexcept { switch (3) { case 1: throw 15; case 3: return; } throw 16; }
void f15() noexcept { switch (2) { case 1: throw 17; default: throw 18; } }
void f16() noexcept { goto L; M: throw 19; L: goto M; }
void f17() noexcept { __builtin_unreachable(); throw 20; }
void f18(int x) noexcept { x && (throw 21, true); }
void f19() noexcept { try { try { throw 1; } catch (long) {} } catch (...) { throw 22; } }
void f20() noexcept { try { try { throw 1; } catch (...) {} } catch (...) { throw 23; } }
void f21() noexcept { const int n = 0; if (n) throw 24; }
void f22() noexcept { do { continue; } while (false); throw 25; }
void f23() noexcept { if constexpr (k) { throw 26; } else { throw 27; } }
void f24(int x) noexcept { x ? die() : die(); throw 28; }
void f25() noexcept { ({ return; }); throw 29; }
void f26() noexcept { int x = (throw 30, 0); throw 31; }
void f27(bool c) noexcept { if (c || true) return; throw 32; }
void f28(int x) noexcept { if (x > 5 && x < 3) throw 33; }
void f29(int x) noexcept { if (x > 2 && x < 5) throw 34; }
void f30() noexcept { L: throw 35; goto L; }
void f31() noexcept { throw 36; if consteval { } }
void f32() noexcept { int i = 0; while (i < 3) { if (i == 2) goto out; ++i; } return; out: throw 37; }
void f33() noexcept { for (int i = 0;; ++i) { if (i) continue; return; } throw 38; }
int main() {}
