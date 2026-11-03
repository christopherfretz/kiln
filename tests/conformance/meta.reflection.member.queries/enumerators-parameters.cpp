// [meta.reflection.member.queries]/13-14 (P2996R13) enumerators_of: the
// enumerators of an enumeration type with a definition (an opaque
// declaration has none to list), in order, through a type alias too.
// [meta.reflection.queries] parameters_of (P3096R12): a function's
// parameters, including the explicit object parameter, or a function
// type's parameter types.
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

enum Color { red, green = 5, blue };
enum class Scoped : char { a, b };
enum Empty {};
using Alias = Color;
static_assert(enumerators_of(^^Color).size() == 3);
static_assert(enumerators_of(^^Color)[0] == ^^red);
static_assert(enumerators_of(^^Color)[2] == ^^blue);
static_assert(enumerators_of(^^Alias)[1] == ^^green);
static_assert(enumerators_of(^^Scoped).size() == 2);
static_assert(enumerators_of(^^Scoped)[1] == ^^Scoped::b);
static_assert(enumerators_of(^^Empty).size() == 0);
static_assert(type_of(enumerators_of(^^Scoped)[0]) == ^^Scoped);

void f(int a, double b, char);
void g();
struct S {
  void m(this S self, int);
  void n(long) const;
};
static_assert(parameters_of(^^f).size() == 3);
static_assert(parameters_of(^^f)[0] != parameters_of(^^f)[1]);
static_assert(parameters_of(^^f)[1] == parameters_of(^^f)[1]);
static_assert(parameters_of(^^g).size() == 0);
static_assert(parameters_of(^^S::m).size() == 2);
static_assert(parameters_of(^^S::n).size() == 1);

// A function type: its parameter types.
static_assert(parameters_of(^^void(int, char*)).size() == 2);
static_assert(parameters_of(^^void(int, char*))[1] == ^^char*);
static_assert(parameters_of(type_of(^^f))[0] == ^^int);

int main() {}
