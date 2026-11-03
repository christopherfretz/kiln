// [meta.reflection.extract] (P2996R13), spec/reflection-values.md §2:
// extract<T&> of a variable or an object (extract-ref): a reference to
// the object itself, binding by at most a qualification conversion
// (array bounds level by level, cv on the element).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

constexpr int g = 42;
static_assert(&extract<const int&>(^^g) == &g);
static_assert(&extract<const volatile int&>(^^g) == &g);
static_assert(extract<const int&>(reflect_object(g)) == 42);
static_assert(reflect_object(g) == reflect_object(g));
static_assert(reflect_object(g) != ^^g);

int h = 7;
static_assert(&extract<int&>(^^h) == &h);
static_assert(&extract<const int&>(reflect_object(h)) == &h);

int marr[] = {3, 1, 4};
static_assert(&extract<int(&)[3]>(reflect_object(marr))[0] == marr);
static_assert(&extract<const int(&)[3]>(^^marr)[1] == marr + 1);
static_assert(&extract<int&>(reflect_object(marr[1])) == marr + 1);

struct P { int x, y; };
constexpr P cp{3, 4};
static_assert(&extract<const P&>(^^cp) == &cp);
static_assert(&extract<const int&>(reflect_object(cp.y)) == &cp.y);

// A class constant is an object: a template parameter object.
static_assert(extract<const P&>(reflect_constant(P{1, 2})).x == 1);
static_assert(&extract<const P&>(reflect_constant(P{1, 2})) ==
              &extract<const P&>(reflect_constant(P{1, 2})));

struct S { static int sm; };
int S::sm = 9;
static_assert(&extract<int&>(^^S::sm) == &S::sm);

// A reference variable: the object it refers to.
constexpr const int& rg = g;
static_assert(&extract<const int&>(^^rg) == &g);
static_assert(extract<int>(^^rg) == 42);
constexpr int& rh = h;
static_assert(&extract<int&>(^^rh) == &h);
constexpr const int (&ra)[3] = marr;
static_assert(&extract<const int(&)[3]>(^^ra)[0] == marr);

int main() {}
