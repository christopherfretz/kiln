// [expr.prim.lambda.closure]/2, [over.over]/1, [expr.static.cast]/4,
// [expr.cast]/4, [over.best.ics]/1, [temp.deduct.call]/6,
// [dcl.type.auto.deduct]/4, [dcl.init.list]/3: clang keeps a function
// template-id an overload set and resolves it again at every step that
// meets it, each resolution numbering the return type's lambda once more
// (spec/sema-lambda.md §3.5). With the early resolution and the
// specialization, `fX<int>` takes: 3 for a `static_cast` or C-style cast
// to a pointer (initialization check and perform step), a qualified
// call's argument, a reference parameter, `auto &`; 2 for a
// `static_cast` to a reference (bound once); 4 for a braced scalar or
// aggregate element (the list checker's verifying pass), `auto *` and an
// `auto` template argument (the deduction); 5 for a deduced `F` (the
// deduction and the candidate's conversion sequence).
// Inputs/lambda-in-signature-explicit-args-conversions-defs.cpp calls the
// `long` specializations' closures under clang's names, so the program
// links only if kiln numbers them alike (WP-1142).
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-in-signature-explicit-args-conversions-defs.cpp
//@ stdout: 11 11
#include <cstdio>
template <class T> struct is_int { static constexpr bool v = false; };
template <> struct is_int<int> { static constexpr bool v = true; };
template <class T, class L> auto pick(L l) {
  if constexpr (is_int<T>::v) return 0; else return l;
}
#define DEF(n) template <class T> auto n(T) -> decltype(pick<T>([] { return 1; })) { return {}; }
DEF(fs) DEF(fc) DEF(fr) DEF(fg) DEF(fh) DEF(fd) DEF(fl) DEF(fk) DEF(fu) DEF(fv) DEF(fn)
using FP = int (*)(int);
using FR = int (&)(int);
int take(FP f) { return f(0); }
int takeref(FR f) { return f(0); }
template <class F> int deduce(F f) { return f(0); }
struct S { FP m; };
int other_conversions();
template <auto> struct TA { static constexpr int v = 0; };
int main() {
  int r = static_cast<FP>(fs<int>)(0) + ((FP)fc<int>)(0) + static_cast<FR>(fr<int>)(0);
  r += ::take(fg<int>) + takeref(fh<int>) + deduce(fd<int>);
  FP l{fl<int>};
  S k{fk<int>};
  auto *u = fu<int>;
  auto &v = fv<int>;
  r += l(0) + k.m(0) + u(0) + v(0) + TA<&fn<int>>::v;
  r += fs<long>(1)() + fc<long>(1)() + fr<long>(1)() + fg<long>(1)() + fh<long>(1)() +
       fd<long>(1)() + fl<long>(1)() + fk<long>(1)() + fu<long>(1)() + fv<long>(1)() +
       fn<long>(1)();
  std::printf("%d %d\n", r, other_conversions());
}
