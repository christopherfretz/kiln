// [dcl.init.general]/6.2-6.3, /7, [class.base.init]/9, [expr.new]/23
// (P2996R13): every default-initialized `std::meta::info` is the null
// reflection: array elements, a member a trivial or a user-provided
// constructor leaves without a mem-initializer, a `new info` and the
// elements of `new info[n]` and `new S[n]`, and a `const info` member
// (`info` is const-default-constructible, /7.1, so the class is too,
// /7.4.1, and its default constructor is not deleted). Other members of
// those classes stay uninitialized. kiln only: GCC 16.1 zero-initializes
// only a variable of type `info` (arrays, members and new-expressions are
// "uninitialized") and rejects a non-constexpr `info` variable (the
// P2996R13 type rule, docs/status/deviations.md), and clang does not
// implement reflection.
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using std::meta::info;

constexpr info arr[3];
static_assert(arr[0] == info{} && arr[2] == info{});
constexpr info arr2[2][2];
static_assert(arr2[1][1] == info{});
constinit info ci; // P4101R1: a null reflection is not consteval-only

struct S {
  info m;
};
constexpr S cs;
static_assert(cs.m == info{});

struct W {
  S a[2];
  const info c;
  info d[2];
};
constexpr W w;
static_assert(w.a[1].m == w.c && w.d[1] == info{});

struct U {
  int i = 1;
  info m;
  consteval U() {}
};

struct Base {
  info b;
};
struct Derived : Base {
  info d;
  consteval Derived() : Base() {}
};

consteval bool locals() {
  S s;
  U u;
  W lw;
  Derived dv;
  info la[4];
  return s.m == info{} && u.m == info{} && u.i == 1 && lw.d[0] == info{} && dv.b == info{} &&
         dv.d == info{} && la[3] == info{};
}
static_assert(locals());

consteval bool heap(int n) {
  info *p = new info;
  bool ok = *p == info{};
  delete p;
  info *q = new info[n];
  ok = ok && q[n - 1] == info{};
  delete[] q;
  S *s = new S[n];
  ok = ok && s[n - 1].m == info{};
  delete[] s;
  return ok;
}
static_assert(heap(3));
