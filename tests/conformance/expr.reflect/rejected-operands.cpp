// [expr.reflect]/3-5 P2996R13: the operand of `^^` is `::`, a type-id, a
// namespace or template name, or an id-expression naming a single
// entity. An overload set (several functions or function templates, the
// constructors of a class), a non-type template parameter, an
// expression in parentheses and a literal are not reflectable.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 25:48 error "'^^' cannot be applied to a non-type template parameter 'N'"
//@ diag: 27:22 error "cannot take the reflection of an overload set"
//@ diag: 28:22 error "cannot take the reflection of an overload set"
//@ diag: 29:25 error "cannot take the reflection of an overload set"
//@ diag: 30:25 error "cannot take the reflection of an overload set"
//@ diag: 31:22 error "'^^' cannot be applied to this operand"
//@ diag: 32:22 error "'^^' cannot be applied to this operand"
//@ diag: 33:22 error "use of undeclared identifier 'undeclared'"
#include "kiln_meta.h"
using namespace std::meta;
void g();
void g(int);
template <class T> void tf();
template <class T> void tf(T);
struct S { void m(); void m(int); S(); };
template <int N> consteval info f() { return ^^N; }
int x;
constexpr info a = ^^g;
constexpr info b = ^^tf;
constexpr info c = ^^S::m;
constexpr info d = ^^S::S;
constexpr info e = ^^(x);
constexpr info h = ^^42;
constexpr info i = ^^undeclared;
