// [meta.reflection.annotation] (P3394R4): annotations_of(item) lists the
// annotations of every declaration of item, first declaration first and
// each in source order; annotations_of_with_type keeps those whose type
// is the given type ignoring const; is_annotation is true of
// exactly these reflections. The IS example's counts and identities are
// kept verbatim; its value checks (extract, constant_of) are WP-479's.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

// The IS example (counts and identities).
[[=1]] void f();
[[=2, =3]] void g();
void g [[=4]] ();
static_assert(annotations_of(^^f).size() == 1);
static_assert(annotations_of(^^g).size() == 3);

struct Option { bool value; };
struct C {
  [[=Option{true}]] int a;
  [[=Option{false}]] int b;
};
static_assert(annotations_of(^^C::a).size() == 1);
static_assert(annotations_of_with_type(^^C::b, ^^Option).size() == 1);

template<class T>
  struct [[=42]] D { };
static_assert(annotations_of(^^D<int>)[0] != annotations_of(^^D<char>)[0]);

[[=1]] int x, y;
static_assert(annotations_of(^^x)[0] == annotations_of(^^y)[0]);

// Not annotations.
static_assert(is_annotation(annotations_of(^^f)[0]));
static_assert(!is_annotation(^^f) && !is_annotation(info()) && !is_annotation(^^int));

// Declaration order across redeclarations, by type.
struct [[=1]] S;
struct [[=2L, =3]] S {};
static_assert(annotations_of(^^S).size() == 3);
static_assert(annotations_of_with_type(^^S, ^^int).size() == 2);
static_assert(annotations_of_with_type(^^S, ^^long).size() == 1);
static_assert(annotations_of_with_type(^^S, ^^int)[0] == annotations_of(^^S)[0]);
static_assert(annotations_of_with_type(^^S, ^^int)[1] == annotations_of(^^S)[2]);
static_assert(annotations_of_with_type(^^S, ^^short).size() == 0);

// The type argument's const is removed (remove_const), not its
// volatile; an alias of it works.
using Int = const int;
static_assert(annotations_of_with_type(^^S, ^^const int).size() == 2);
static_assert(annotations_of_with_type(^^S, ^^Int).size() == 2);
static_assert(annotations_of_with_type(^^S, ^^volatile long).size() == 0);

// A type alias has its own annotations, not its target's.
using A [[=5]] = S;
static_assert(annotations_of(^^A).size() == 1);
static_assert(annotations_of_with_type(^^A, ^^int)[0] != annotations_of(^^S)[0]);

// Namespaces, variables, enumerators, non-static data members.
namespace [[=1]] n {}
namespace [[=2]] n {}
static_assert(annotations_of(^^n).size() == 2);
extern int v [[=1]];
[[=2]] int v = 0;
static_assert(annotations_of(^^v).size() == 2);
enum E { e0 [[=1]], e1 };
static_assert(annotations_of(^^e0).size() == 1 && annotations_of(^^e1).size() == 0);
static_assert(annotations_of(^^E).size() == 0);

// Unannotated entities and built-in types have none.
void plain();
static_assert(annotations_of(^^plain).size() == 0);
static_assert(annotations_of(^^int).size() == 0);

// Function parameters (each declaration's parameter).
void p([[=1]] int a, int b);
void p(int a, [[=2]] int b);
static_assert(annotations_of(parameters_of(^^p)[0]).size() == 1);
static_assert(annotations_of(parameters_of(^^p)[1]).size() == 1);

// A function template specialization takes its template's annotations.
template <int N> [[=N]] int t() { return N; }
static_assert(annotations_of(^^t<3>).size() == 1);

int main() {}
