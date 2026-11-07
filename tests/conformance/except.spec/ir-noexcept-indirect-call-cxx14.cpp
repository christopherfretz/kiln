// [except.spec]/1, /13 (C++14, pre-P0012): before C++17 a pointer's or
// reference's pointee as written keeps its exception specification, so a
// call through a `noexcept` function pointer, reference or member
// function pointer (declared, `auto`, a typedef of `decltype(&f)`, a
// member) cannot throw and is `nounwind`, as clang emits it; a template
// argument keeps none, so a call through a deduced `F` may throw. kiln
// read the canonical pointee, which has no specification (WP-751).
//@ expect: ir
//@ min-oracle: 22
//@ std: c++14
void f() noexcept;
typedef decltype(&f) TP;
using UF = decltype(f);
struct H { void (*p)() noexcept; void m() noexcept; };
void (&r)() noexcept = f;
TP tp = f;
UF *up = f;
H h{f};
template <class F> void call(F x) { x(); }
void k1() { r(); }
void k2() { tp(); }
void k3() { up(); }
void k4() { auto p = &f; p(); }
void k5() { h.p(); }
void k6() { call(&f); }
void k7() { auto m = &H::m; (h.*m)(); }
void k8() { (*tp)(); }
void k9() { decltype(&f) d = f; d(); }
void k10() { auto &cr = tp; cr(); }
