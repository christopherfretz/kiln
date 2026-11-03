// [meta.reflection.extract] (P2996R13), spec/reflection-values.md §2:
// extract<T> of a value (extract-value): the value itself when T is its
// type ignoring cv, a pointer by a qualification conversion, a pointer
// to the first element of the reflect_constant_array object of an array
// (constant_of), an enumerator's value, and a class constant copied whole.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

static_assert(extract<int>(reflect_constant(7)) == 7);
static_assert(extract<const int>(reflect_constant(5)) == 5);
static_assert(extract<long>(reflect_constant(7L)) == 7);
static_assert(extract<char>(reflect_constant('a')) == 'a');
static_assert(extract<bool>(reflect_constant(true)));
static_assert(extract<double>(reflect_constant(2.5)) == 2.5);
static_assert(extract<decltype(nullptr)>(reflect_constant(nullptr)) == nullptr);

// A variable: its value (constant_of(r) is reflect_constant of it).
constexpr int g = 42;
static_assert(extract<int>(^^g) == 42);
static_assert(extract<int>(reflect_object(g)) == 42);

// Pointers: similar types by a qualification conversion.
int h = 7;
static_assert(extract<int*>(reflect_constant(&h)) == &h);
static_assert(extract<const int*>(reflect_constant(&h)) == &h);
static_assert(extract<const volatile int*>(reflect_constant(&h)) == &h);

// Function pointers, including a closure's conversion result.
int f(int a) { return a + 1; }
static_assert(extract<int (*)(int)>(reflect_constant(&f)) == &f);
static_assert(reflect_constant(&f) != ^^f);
constexpr auto lam = [](int a) { return a * 2; };
static_assert(extract<int (*)(int)>(reflect_constant(+lam))(4) == 8);

// Enumerators.
enum E { e1 = 5 };
enum class EC : unsigned char { a = 3 };
static_assert(extract<E>(^^e1) == e1);
static_assert(extract<EC>(^^EC::a) == EC::a);

// Class constants (template parameter objects) and class variables.
struct P { int x, y; };
static_assert(extract<P>(reflect_constant(P{1, 2})).y == 2);
constexpr P cp{3, 4};
static_assert(extract<P>(^^cp).x == 3);
static_assert(extract<const P>(reflect_object(cp)).y == 4);

// Arrays: T is a pointer to const element; the pointer is into the
// content-interned reflect_constant_array object, not the variable.
constexpr int arr[] = {3, 1, 4};
static_assert(extract<const int*>(^^arr) != arr);
static_assert(extract<const int*>(^^arr) == extract<const int*>(reflect_constant_array(arr)));
static_assert(extract<const int*>(reflect_object(arr)) == std::define_static_array(arr).data());
static_assert(extract<const int*>(^^arr)[2] == 4);

int main() {}
