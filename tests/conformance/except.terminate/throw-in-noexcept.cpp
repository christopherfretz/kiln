// [except.terminate]/1, [except.spec]/5: a throw-expression that no
// handler of the function catches, in a function with a non-throwing
// exception specification, calls std::terminate; clang warns
// (-Wexceptions) once per such throw, in reverse source order. Templates
// are checked at instantiation, not as patterns.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 22:21 warning "'f' has a non-throwing exception specification but can still throw"
//@ diag: 22:6 note "function declared non-throwing here"
//@ diag: 25:54 warning "'l' has a non-throwing exception specification but can still throw"
//@ diag: 25:6 note "function declared non-throwing here"
//@ diag: 25:27 warning "'l' has a non-throwing exception specification but can still throw"
//@ diag: 25:6 note "function declared non-throwing here"
//@ diag: 26:26 warning "'m' has a non-throwing exception specification but can still throw"
//@ diag: 26:6 note "function declared non-throwing here"
//@ diag: 27:58 warning "'o' has a non-throwing exception specification but can still throw"
//@ diag: 27:6 note "function declared non-throwing here"
//@ diag: 27:33 warning "'o' has a non-throwing exception specification but can still throw"
//@ diag: 27:6 note "function declared non-throwing here"
struct B {};
struct D : B {};
void f() noexcept { throw 1; }
void g() noexcept { try { throw 1; } catch (...) {} }
void k() noexcept { try { throw D(); } catch (B &) {} }
void l() noexcept { try { throw D(); } catch (int) { throw; } }
void m() throw() { try { throw 1; } catch (long) {} }
void o(int a) noexcept { if (a) throw 1; else { int b = (throw 2, 3); } }
void p() noexcept { int *q = 0; try { throw q; } catch (const int *) {} }
void r() noexcept(false) { throw 1; }
template <class T> void t() noexcept { throw T(); }
template <class T> struct Q { void f() noexcept { throw 1; } };
