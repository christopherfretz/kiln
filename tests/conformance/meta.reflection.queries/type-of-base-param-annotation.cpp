// [meta.reflection.queries] (P2996R13 type_of, P3096R12, P3394R4): a
// direct base class relationship has the type of its base class, a
// function parameter its declared type, an annotation the type of its
// underlying constant; and (P3032R2) the element of the vector
// annotations_of returns can initialize a constexpr variable: the vector
// itself is destroyed in the initializer.
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
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

struct B {};
struct D : B {};
void f(int x, const char* y);
struct [[=42, ='c']] A {};
constexpr auto ctx = access_context::unchecked();
static_assert(type_of(bases_of(^^D, ctx)[0]) == ^^B);
static_assert(type_of(parameters_of(^^f)[0]) == ^^int);
static_assert(type_of(parameters_of(^^f)[1]) == ^^const char*);
static_assert(type_of(annotations_of(^^A)[0]) == ^^int);
static_assert(type_of(annotations_of(^^A)[1]) == ^^char);
constexpr info v = annotations_of(^^A)[0];
static_assert(extract<int>(v) == 42);
constexpr auto w = annotations_of(^^A)[1];
static_assert(extract<char>(w) == 'c');
