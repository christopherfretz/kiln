// [meta.reflection.result] (P2996R13), spec/reflection-values.md §3:
// reflect_constant, reflect_object and reflect_function, and the identity
// of what they return. reflect_constant of a scalar is a reflection of a
// value, of a class type a reflection of a template parameter object, so
// two calls are equal exactly when the template arguments TCls<V> would
// be template-argument-equivalent; reflect_object and reflect_function
// designate the entity itself.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

// The IS example ([meta.reflection.result] Example 1), without its
// is_value/is_object/template_arguments_of lines (queries, WP-476) and
// the ill-formed r4 (reflect-neg.cpp).
template <auto D>
struct A {};
struct N { int x; };
struct K { char const* p; };
constexpr info r1 = reflect_constant(42);
constexpr info r2 = reflect_constant(N{42});
constexpr info r3 = reflect_constant(K{nullptr});
static_assert(r1 != r2 && r2 != r3);

// Values: equal by value and type.
static_assert(reflect_constant(1) == reflect_constant(1));
static_assert(reflect_constant(1) != reflect_constant(2));
static_assert(reflect_constant(1) != reflect_constant(1L));
static_assert(reflect_constant(1) != reflect_constant(1u));
static_assert(reflect_constant('a') != reflect_constant(97));
static_assert(reflect_constant(1.5) == reflect_constant(1.5));
static_assert(reflect_constant(nullptr) == reflect_constant(nullptr));
static_assert(reflect_constant(^^int) == reflect_constant(^^int));
static_assert(reflect_constant(^^int) != ^^int);
constexpr int ci = 3;
static_assert(reflect_constant(ci) == reflect_constant(3));
static_assert(reflect_constant(r1) == reflect_constant(reflect_constant(42)));
enum E { e1 = 5 };
static_assert(reflect_constant(e1) != reflect_constant(5));
static_assert(reflect_constant(e1) == reflect_constant(E(5)));
struct M { int m, n; };
static_assert(reflect_constant(&M::m) == reflect_constant(&M::m));
static_assert(reflect_constant(&M::m) != reflect_constant(&M::n));

// Objects of class type: template-argument-equivalence.
static_assert(reflect_constant(N{42}) == r2);
static_assert(reflect_constant(N{41}) != r2);
constexpr N cn{42};
static_assert(reflect_constant(cn) == r2);
static_assert(extract<const N&>(r2).x == 42);
static_assert(&extract<const N&>(r2) == &extract<const N&>(reflect_constant(N{42})));
struct P { int x, y; };
static_assert(reflect_constant(P{1, 2}) == reflect_constant(P{1, 2}));
static_assert(reflect_constant(P{1, 2}) != reflect_constant(P{2, 1}));
static_assert(extract<P>(reflect_constant(P{1, 2})).y == 2);

// Pointers: by the object or function they designate.
int h = 7;
constexpr int f(int a) { return a + 1; }
static_assert(reflect_constant(&h) == reflect_constant(&h));
static_assert(extract<int*>(reflect_constant(&h)) == &h);
static_assert(reflect_constant(&f) == reflect_constant(&f));
static_assert(reflect_constant(&f) != ^^f);
static_assert(reflect_constant(K{nullptr}) == r3);

// reflect_object: the object, not its value.
constexpr int g = 1;
static_assert(reflect_object(g) == reflect_object(g));
static_assert(reflect_object(g) != ^^g);
static_assert(reflect_object(g) != reflect_constant(g));
static_assert(&extract<const int&>(reflect_object(g)) == &g);
int marr[] = {3, 1, 4};
static_assert(reflect_object(marr[1]) == reflect_object(marr[1]));
static_assert(reflect_object(marr[1]) != reflect_object(marr[2]));
static_assert(&extract<int&>(reflect_object(marr[1])) == marr + 1);
static_assert(&extract<int(&)[3]>(reflect_object(marr))[0] == marr);
struct S { int a; static int sm; };
static_assert(&extract<int&>(reflect_object(S::sm)) == &S::sm);
constexpr S cs{4};
static_assert(reflect_object(cs.a) != reflect_object(cs));
static_assert(extract<int>(reflect_object(cs.a)) == 4);

// reflect_function: the function, the same reflection as ^^f.
static_assert(reflect_function(f) == ^^f);
static_assert(extract<int (*)(int)>(reflect_function(f))(1) == 2);
