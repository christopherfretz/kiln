// [meta.reflection.queries] P2996R13 constant_of of a value of any
// structural type: with R the splice of r, reflect_constant_array(R) of
// an array, reflect_function(R) of a function, reflect_constant(R) of
// anything else ([meta.reflection.result], P3491R3). A floating-point
// value, a pointer, a pointer to member or a reflection is a value
// reflection; a class value is its template parameter object; an array
// value is the reflect_constant_array object holding its elements. A
// reference's constant is the object's it refers to; a (sub)object's is
// read from its complete object (a variable usable in constant
// expressions, a template parameter object, a reflect_constant_array
// object); a structured binding's is its binding expression's.
// Checked against GCC 16.1 (-freflection), which agrees except where
// spec/reflection-queries.md §5 says: GCC makes constant_of of an array
// a variable reflection, not content-interned (lines 55, 60 and 89
// fail there).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ xfail(gcc): GCC 16.1's constant_of of an array, and reflect_constant/reflect_object of these values, differ
#include "kiln_meta_queries.h"
using namespace std::meta;

struct In { int a; char c; };
struct S { int m; double d; In in; int arr[2]; };
constexpr S s{42, 1.5, {7, 'x'}, {8, 9}};
constexpr double dd = 2.5;
constexpr float ff = 0.5f;
constexpr int arr[3] = {1, 2, 3};
constexpr S sarr[2] = {{1, 0.0, {2, 'a'}, {3, 4}}, {5, 0.5, {6, 'b'}, {7, 8}}};
constexpr const int& ri = arr[1];
constexpr const S& rs = s;
constexpr int ci = 4;
constexpr const int& rci = ci;
constexpr int* np = nullptr;
constexpr const int* pa = &arr[2];
constexpr int S::* pm = &S::m;
constexpr info ti = ^^int;
constexpr const char (&sr)[3] = "ab";
void fn();

// A floating-point value: a value reflection of its type.
static_assert(is_value(constant_of(^^dd)) && type_of(constant_of(^^dd)) == ^^double);
static_assert(constant_of(^^dd) == reflect_constant(2.5));
static_assert(type_of(constant_of(^^ff)) == (^^float) && constant_of(^^ff) == reflect_constant(0.5f));
static_assert(extract<double>(constant_of(^^dd)) == 2.5);

// A class value: its template parameter object, of type const S.
static_assert(is_object(constant_of(^^s)) && type_of(constant_of(^^s)) == ^^const S);
static_assert(constant_of(^^s) == reflect_constant(S{42, 1.5, {7, 'x'}, {8, 9}}));
static_assert(constant_of(^^s) != object_of(^^s));
static_assert(constant_of(constant_of(^^s)) == constant_of(^^s));
static_assert(extract<S>(constant_of(^^s)).in.c == 'x');
static_assert(extract<const S&>(constant_of(^^s)).arr[1] == 9);

// An array: reflect_constant_array of its elements.
static_assert(is_object(constant_of(^^arr)) && type_of(constant_of(^^arr)) == ^^const int[3]);
static_assert(constant_of(^^arr) == reflect_constant_array(arr));
static_assert(constant_of(constant_of(^^arr)) == constant_of(^^arr));
static_assert(extract<const int*>(constant_of(^^arr))[2] == 3);
static_assert(constant_of(^^sarr) == reflect_constant_array(sarr));
static_assert(constant_of(^^sr) == reflect_constant_array(sr));

// A reference: the constant of the object it refers to.
static_assert(constant_of(^^ri) == reflect_constant(2));
static_assert(constant_of(^^rs) == constant_of(^^s));
static_assert(constant_of(^^rci) == constant_of(^^ci));
static_assert(constant_of(object_of(^^ri)) == reflect_constant(2));

// Pointers, a pointer to member and a reflection.
static_assert(constant_of(^^np) == reflect_constant(np) && type_of(constant_of(^^np)) == ^^int*);
static_assert(constant_of(^^pa) == reflect_constant(pa) && type_of(constant_of(^^pa)) == ^^const int*);
static_assert(constant_of(^^pm) == reflect_constant(&S::m));
static_assert(constant_of(^^ti) == reflect_constant(^^int));

// Subobjects of a constant: their values.
static_assert(constant_of(reflect_object(s.m)) == reflect_constant(42));
static_assert(constant_of(reflect_object(s.d)) == reflect_constant(1.5));
static_assert(constant_of(reflect_object(s.in)) == reflect_constant(In{7, 'x'}));
static_assert(constant_of(reflect_object(s.in.c)) == reflect_constant('x'));
static_assert(constant_of(reflect_object(s.arr)) == reflect_constant_array(s.arr));
static_assert(constant_of(reflect_object(arr[2])) == reflect_constant(3));
static_assert(constant_of(reflect_object(sarr[1])) == reflect_constant(sarr[1]));
static_assert(constant_of(reflect_object(sarr[1].in.a)) == reflect_constant(6));

// A template parameter object and a reflect_constant_array object are
// their own constants; their subobjects' constants are values.
static_assert(constant_of(reflect_constant(In{1, 'q'})) == reflect_constant(In{1, 'q'}));
static_assert(constant_of(reflect_object(extract<const S&>(constant_of(^^s)).in.c)) ==
              reflect_constant('x'));
static_assert(constant_of(reflect_object(extract<const int*>(constant_of(^^arr))[1])) ==
              reflect_constant(2));

// A function: reflect_function of it, the function itself.
struct F { static int sf(); static constexpr int sm = 9; };
static_assert(constant_of(^^fn) == ^^fn);
static_assert(constant_of(^^F::sf) == ^^F::sf);
static_assert(constant_of(^^F::sm) == reflect_constant(9));

// Structured bindings of a constexpr decomposition: their values.
constexpr auto [b1, b2, b3, b4] = s;
static_assert(constant_of(^^b1) == reflect_constant(42));
static_assert(constant_of(^^b3) == reflect_constant(In{7, 'x'}));
constexpr auto [e1, e2, e3] = arr;
static_assert(constant_of(^^e3) == reflect_constant(3));

// A constexpr local variable of a consteval function.
consteval bool local() {
  constexpr S l{3, 0.25, {1, 'l'}, {0, 0}};
  constexpr double ld = 0.75;
  return constant_of(^^l) == reflect_constant(S{3, 0.25, {1, 'l'}, {0, 0}}) &&
         constant_of(^^ld) == reflect_constant(0.75);
}
static_assert(local());

int main() {}
