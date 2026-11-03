// [meta.reflection.queries], [meta.reflection.operators] P2996R13 and
// P3096R12: the failure conditions of operator_of (not an operator
// function or operator function template: a plain function, a
// conversion function, a member function), symbol_of / u8symbol_of (a
// value that is no enumerator of std::meta::operators), return_type_of
// (neither a function nor a function type) and variable_of (not a
// parameter of the function whose definition is being evaluated; a
// variable named in that function is not a parameter reflection). Each
// note is in the shape of GCC 16.1's std::meta::exception what() for
// the same call (checked by hand).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:15 note "reflection does not represent an operator function or operator function template"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 41:15 note "reflection does not represent an operator function or operator function template"
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:15 note "reflection does not represent an operator function or operator function template"
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:15 note "operators argument is not a valid operator"
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:15 note "operators argument is not a valid operator"
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:15 note "operators argument is not a valid operator"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:15 note "reflection does not represent a function or function type with a return type"
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:15 note "reflection does not represent a function or function type with a return type"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 48:15 note "reflection does not represent parameter of current function"
//@ diag: 50:17 error "static assertion expression is not an integral constant expression"
//@ diag: 50:17 note "reflection does not represent parameter of current function"
#include "kiln_meta_queries.h"
using namespace std::meta;
struct S { int m; operator int() const; void f(); };
int gv;
void fn(int);
static_assert(operator_of(^^fn) == op_new);
static_assert(operator_of(members_of(^^S, access_context::unchecked())[1]) == op_new);
static_assert(operator_of(^^S::f) == op_new);
static_assert(symbol_of(operators(0))[0] == 0);
static_assert(symbol_of(operators(45))[0] == 0);
static_assert(u8symbol_of(operators(-1))[0] == 0);
static_assert(return_type_of(^^int) == ^^int);
static_assert(return_type_of(^^gv) == ^^int);
static_assert(variable_of(^^gv) == ^^gv);
void h(int p) {
  static_assert(variable_of(^^p) == ^^p);
}
