// [dcl.type.splice] Example 1, verbatim, P2996R13: a splice-type-specifier
// needs `typename` unless it appears in a type-only context
// ([temp.res.general]/4); without it, at block scope the splice is a
// splice-expression, and one designating a type is ill-formed.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 19:5 error "splice in an expression position does not designate a variable, function, enumerator, value or object"
//@ diag: 19:18 error "use of undeclared identifier 'var'"
#include "kiln_meta.h"
template<std::meta::info R> void tfn() {
  typename [:R:]::type m;       // OK, typename applies to the qualified name
}

struct S { using type = int; };

void fn() {
  [:^^S::type:] *var;           // error: [:^^S::type:] is an expression
  typename [:^^S::type:] *var;  // OK, declares variable with type int*
}

using alias = [:^^S::type:];    // OK, type-only context

void use() { tfn<^^S>(); }
static_assert(__is_same(alias, int));
