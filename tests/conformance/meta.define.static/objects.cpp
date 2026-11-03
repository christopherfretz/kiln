// [meta.define.static] (P3491R3), spec/reflection-values.md §4:
// define_static_object. A class or union value is promoted as the
// template parameter object reflect_constant would make; an array by
// reflect_constant_array; any other value as the one element of a
// define_static_array. Equal values give the same object.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

// Classes: the template parameter object of the value.
struct P { int x, y; };
constexpr const P* po = std::define_static_object(P{5, 6});
static_assert(po->x == 5 && po->y == 6);
static_assert(std::define_static_object(P{5, 6}) == po);
static_assert(std::define_static_object(P{6, 5}) != po);
constexpr P cp{5, 6};
static_assert(std::define_static_object(cp) == po);
static_assert(po == &extract<const P&>(reflect_constant(P{5, 6})));
static_assert(po != &cp);
struct Nested { P p; int a[2]; char c; };
constexpr auto no = std::define_static_object(Nested{{1, 2}, {3, 4}, 'z'});
static_assert(no->p.y == 2 && no->a[1] == 4 && no->c == 'z');

// Unions: by the active member.
union U { int i; float f; };
constexpr const U* uo = std::define_static_object(U{.i = 7});
static_assert(uo->i == 7);
static_assert(std::define_static_object(U{.i = 7}) == uo);
static_assert(std::define_static_object(U{.f = 1.0f}) != uo);

// Arrays: the reflect_constant_array object.
constexpr int arr[] = {3, 1, 4};
constexpr const int (*ao)[3] = std::define_static_object(arr);
static_assert((*ao)[2] == 4);
static_assert(&(*ao)[0] == std::define_static_array(arr).data());

// Scalars: the element of a one-element array.
constexpr int ci = 3;
constexpr const int* qi = std::define_static_object(ci);
static_assert(*qi == 3);
static_assert(qi == std::define_static_object(3));
static_assert(qi != &ci);
static_assert(*std::define_static_object(2.5) == 2.5);
static_assert(*std::define_static_object(^^int) == ^^int);
static_assert(*std::define_static_object(&ci) == &ci);
constexpr const char* const* so = std::define_static_object(std::define_static_string("s"));
static_assert((*so)[0] == 's');

// A computed value: copied out of a transient one.
consteval const P* make(int n) {
  P* p = new P{n, n + 1};
  const P* r = std::define_static_object(*p);
  delete p;
  return r;
}
static_assert(make(1)->y == 2 && make(1) == make(1));
