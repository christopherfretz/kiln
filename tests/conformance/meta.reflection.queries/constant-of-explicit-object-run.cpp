// [meta.reflection.queries] constant_of of an explicit-object member
// function (P2996R13): the constant is the function itself, so the pointer
// extract gives, and &[: constant_of(r) :], are ordinary pointers to
// function ([expr.unary.op]/3) that code generation emits as the
// function's address and calls at run time (WP-1782). GCC 16.1 compiles
// and runs the same file; the output must agree.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ stdout: 5 7 8 40
//@ stdout: 1 1 1
#include <meta>
#include <cstdio>
using namespace std::meta;

struct S {
  int v;
  int f(this const S& s) { return s.v + 1; }
  int by_value(this S s, int k) { return s.v + k; }
  template <class T> int t(this T&& s) { return s.v * 2; }
  operator int(this const S& s) { return s.v * 10; }
};

constexpr int (*pf)(const S&) = extract<int (*)(const S&)>(constant_of(^^S::f));
constexpr int (*pv)(S, int) = extract<int (*)(S, int)>(constant_of(^^S::by_value));
constexpr int (*pt)(const S&) =
    extract<int (*)(const S&)>(constant_of(substitute(^^S::t, {^^const S&})));
constexpr int (*pc)(const S&) = &[:constant_of(^^S::operator int):];
int (*volatile vf)(const S&) = pf;

int main() {
  S s{4};
  int a = vf(s);
  int b = pv(s, 3);
  int c = pt(s);
  int d = pc(s);
  std::printf("%d %d %d %d\n", a, b, c, d);
  bool same_f = pf == &S::f;
  bool same_t = pt == &S::t<const S&>;
  bool same_c = pc == &S::operator int;
  std::printf("%d %d %d\n", same_f, same_t, same_c);
}
