// [meta.reflection.queries] P2996R13 constant_of of an object reflection
// reads the object's value whatever its type (WP-1743): a class with
// members, an array of classes and an element of one, a pointer to
// member, a reflection, a union with its active member, a closure, a char
// array and a double, each a subobject of a constexpr variable. A class
// value is its template parameter object, an array value the
// reflect_constant_array object of its elements (of type const In[2]),
// any other value a value reflection. GCC 16.1 accepts the same file.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
struct In { int a; char c[3]; };
struct Out { In in; In arr[2]; int In::*pm; std::meta::info r; double d; };
union U { int i; float f; };
struct HU { U u; };
constexpr auto lam = [](int x) { return x + 1; };
struct HL { decltype(lam) l; int k; };
constexpr Out o{{1, {'a', 'b', 'c'}}, {{2, {}}, {3, {'x'}}}, &In::a, ^^int, 2.5};
constexpr HU hu{{.f = 1.5f}};
constexpr HL hl{lam, 7};
using namespace std::meta;
constexpr auto c1 = constant_of(reflect_object(o.in));
constexpr auto c2 = constant_of(reflect_object(o.arr));
constexpr auto c3 = constant_of(reflect_object(o.arr[1]));
constexpr auto c4 = constant_of(reflect_object(o.pm));
constexpr auto c5 = constant_of(reflect_object(o.r));
constexpr auto c6 = constant_of(reflect_object(hu.u));
constexpr auto c7 = constant_of(reflect_object(hl.l));
constexpr auto c8 = constant_of(reflect_object(o.in.c));
constexpr auto c9 = constant_of(reflect_object(o.d));
static_assert([:c1:].a == 1 && [:c1:].c[2] == 'c');
static_assert([:c2:][1].a == 3);
static_assert(type_of(c2) == ^^const In[2]);
static_assert([:c3:].c[0] == 'x');
static_assert([:c4:] == &In::a);
static_assert([:c5:] == ^^int);
static_assert([:c6:].f == 1.5f);
static_assert([:c7:](2) == 3);
static_assert([:c8:][1] == 'b');
static_assert([:c9:] == 2.5);
static_assert(is_object(c1) && is_value(c4) && is_value(c9));
