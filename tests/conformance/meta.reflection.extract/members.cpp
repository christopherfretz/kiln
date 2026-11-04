// [meta.reflection.extract]/6-7 (P2996R13), spec/reflection-values.md §2:
// extract<T> of a function (T is F* for a function of type F or
// F noexcept: non-member, static or explicit object member function) and
// of a non-static member (T is X C::* by a qualification conversion for
// a data member, F C::* for an implicit object member function).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

constexpr int f(int a) { return a + 1; }
void nf() noexcept;
static_assert(extract<int (*)(int)>(^^f) == &f);
static_assert(extract<int (*)(int)>(^^f)(1) == 2);
static_assert(extract<void (*)()>(^^nf) == &nf);
static_assert(extract<void (*)() noexcept>(^^nf) == &nf);
static_assert(reflect_function(f) == ^^f);
static_assert(extract<int (*)(int)>(reflect_function(f)) == &f);

struct S {
  int m;
  const int c = 0;
  static int sf();
  int fn(int) const;
  void g() noexcept;
  int h(this S const&);
};
static_assert(extract<int S::*>(^^S::m) == &S::m);
static_assert(extract<const int S::*>(^^S::m) == &S::m);
static_assert(extract<const int S::*>(^^S::c) == &S::c);
static_assert(extract<int (*)()>(^^S::sf) == &S::sf);
static_assert(extract<int (S::*)(int) const>(^^S::fn) == &S::fn);
static_assert(extract<void (S::*)()>(^^S::g) == &S::g);
static_assert(extract<void (S::*)() noexcept>(^^S::g) == &S::g);
static_assert(extract<int (*)(S const&)>(^^S::h) == &S::h);

constexpr S s{5};
static_assert(s.*extract<int S::*>(^^S::m) == 5);

int main() {}
