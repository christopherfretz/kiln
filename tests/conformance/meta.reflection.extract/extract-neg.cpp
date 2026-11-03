// [meta.reflection.extract] (P2996R13), spec/reflection-values.md §2: every
// condition under which extract<T>(r) throws, each diagnosed with a note
// naming the reflection and T (the evaluation fails; std::meta::exception
// as a thrown object is WP-484's).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-fail
//@ strict-diags
//@ diag: 64:15 error "static assertion expression is not an integral constant expression"
//@ diag: 64:15 note "'^^(value of type int)' cannot be extracted as a 'long'"
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"
//@ diag: 65:15 note "'^^(value of type char)' cannot be extracted as a 'int'"
//@ diag: 66:15 error "static assertion expression is not an integral constant expression"
//@ diag: 66:15 note "'^^e1' cannot be extracted as a 'int'"
//@ diag: 67:15 error "static assertion expression is not an integral constant expression"
//@ diag: 67:15 note "'^^(value of type int *)' cannot be extracted as a 'long *'"
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 68:15 note "'^^(value of type const int *)' cannot be extracted as a 'int *'"
//@ diag: 70:15 error "static assertion expression is not an integral constant expression"
//@ diag: 70:15 note "'^^arr' cannot be extracted as a 'int *'"
//@ diag: 73:15 error "static assertion expression is not an integral constant expression"
//@ diag: 73:15 note "read of non-const variable 'h' is not allowed in a constant expression"
//@ diag: 52:5 note "declared here"
//@ diag: 75:15 error "static assertion expression is not an integral constant expression"
//@ diag: 75:15 note "'^^(value of type int)' is a value, not an object: it cannot be extracted as a reference"
//@ diag: 77:15 error "static assertion expression is not an integral constant expression"
//@ diag: 77:15 note "'^^g' cannot be extracted as a 'int &'"
//@ diag: 78:15 error "static assertion expression is not an integral constant expression"
//@ diag: 78:15 note "'^^h' cannot be extracted as a 'long &'"
//@ diag: 80:15 error "static assertion expression is not an integral constant expression"
//@ diag: 80:16 note "'^^f' is a function, not an object: it cannot be extracted as a reference"
//@ diag: 81:15 error "static assertion expression is not an integral constant expression"
//@ diag: 81:15 note "'^^S::m' is a non-static member, not an object: it cannot be extracted as a reference"
//@ diag: 83:15 error "static assertion expression is not an integral constant expression"
//@ diag: 83:15 note "'^^f' cannot be extracted as a 'int (*)(long)'"
//@ diag: 84:15 error "static assertion expression is not an integral constant expression"
//@ diag: 84:15 note "'^^S::m' cannot be extracted as a 'long S::*'"
//@ diag: 85:15 error "static assertion expression is not an integral constant expression"
//@ diag: 85:15 note "'^^S::m' cannot be extracted as a 'int D::*'"
//@ diag: 86:15 error "static assertion expression is not an integral constant expression"
//@ diag: 86:15 note "'^^S::fn' cannot be extracted as a 'int (S::*)(int)'"
//@ diag: 87:15 error "static assertion expression is not an integral constant expression"
//@ diag: 87:15 note "'^^S::b' is a bit-field: it has no pointer to member"
//@ diag: 89:15 error "static assertion expression is not an integral constant expression"
//@ diag: 89:15 note "'^^int' does not reflect a value, object, variable, function, enumerator or non-static member"
//@ diag: 90:15 error "static assertion expression is not an integral constant expression"
//@ diag: 90:15 note "'std::meta::info()' does not reflect a value, object, variable, function, enumerator or non-static member"
#include "kiln_meta.h"
using namespace std::meta;

constexpr int g = 42;
int h = 7;
constexpr int arr[] = {3, 1, 4};
enum E { e1 = 5 };
int f(int a) { return a; }
struct S {
  int m;
  int b : 3;
  int fn(int) const;
};
struct D : S {};

// extract-value: T and the value's type differ beyond cv.
static_assert(extract<long>(reflect_constant(7)) == 7);
static_assert(extract<int>(reflect_constant('a')) == 97);
static_assert(extract<int>(^^e1) == 5);
static_assert(extract<long*>(reflect_constant(&h)) == nullptr);
static_assert(extract<int*>(reflect_constant(&g)) == nullptr);
// extract-value: an array is extracted as a pointer to const.
static_assert(extract<int*>(^^arr) == nullptr);
// extract-value: constant_of(r) of a variable not usable in constant
// expressions.
static_assert(extract<int>(^^h) == 7);
// extract-ref: a value is no object.
static_assert(extract<const int&>(reflect_constant(7)) == 7);
// extract-ref: a const object binds no reference to non-const.
static_assert(extract<int&>(^^g) == 42);
static_assert(extract<long&>(^^h) == 7);
// extract-ref: a function or member is no object.
static_assert(&extract<int(&)(int)>(^^f) == &f);
static_assert(extract<int&>(^^S::m) == 0);
// extract-member-or-function: the type is not the member's.
static_assert(extract<int (*)(long)>(^^f) == nullptr);
static_assert(extract<long S::*>(^^S::m) == nullptr);
static_assert(extract<int D::*>(^^S::m) == nullptr);
static_assert(extract<int (S::*)(int)>(^^S::fn) == nullptr);
static_assert(extract<int S::*>(^^S::b) == nullptr);
// Not a value, object, variable, function, enumerator or member.
static_assert(extract<int>(^^int) == 0);
static_assert(extract<int>(info()) == 0);
