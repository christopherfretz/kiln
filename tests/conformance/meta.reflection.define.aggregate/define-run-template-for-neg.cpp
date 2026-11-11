// [basic.types.general]/12, [expr.const] (P2996R13), [stmt.expand]: a
// reflection is a consteval-only value, so an expression of type
// std::meta::info outside an immediate function context and outside a
// manifestly constant-evaluated one is ill-formed: in a run-time
// `template for` body, comparing the expansion variable with another
// reflection (define-run-template-for.cpp holds the valid form, the
// comparison in a constexpr variable's initializer), as in a function
// template's body, checked in each expansion and instantiation. The
// first comparison is also an immediate invocation whose result, a
// std::vector, still owns its heap storage, so not a constant
// expression. GCC 16.1 rejects every comparison with the same
// "consteval-only expressions are only allowed in a constant-evaluated
// context", at the same column (it reports each once per expansion and
// once for the template body, kiln once per location, and GCC does not
// name the immediate invocation of line 55, which kiln reports three
// times in clang's wording).
//@ expect: compile-fail
//@ args: -ferror-limit=0 -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 55:23 error "call to consteval function 'std::meta::nonstatic_data_members_of' is not a constant expression"
//@ diag: 55:23 note "pointer to subobject of heap-allocated object is not a constant expression"
//@ diag: 55:48 note "heap allocation performed here"
//@ diag: 55:23 error "call to consteval function 'std::meta::nonstatic_data_members_of' is not a constant expression"
//@ diag: 55:23 note "pointer to subobject of heap-allocated object is not a constant expression"
//@ diag: 55:48 note "heap allocation performed here"
//@ diag: 55:18 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 56:18 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 57:18 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 55:23 error "call to consteval function 'std::meta::nonstatic_data_members_of' is not a constant expression"
//@ diag: 55:23 note "pointer to subobject of heap-allocated object is not a constant expression"
//@ diag: 55:48 note "heap allocation performed here"
//@ diag: 48:16 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 59:3 note "in instantiation of function template specialization 'f<int>' requested here"
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
using namespace std::meta;

struct S;
consteval {
  define_aggregate(^^S, {data_member_spec(^^int, {.name = "a"}),
                         data_member_spec(^^long, {.name = "bb"})});
}

template <class T> void f() {
  constexpr info m = ^^T;
  constexpr info b = ^^long;
  printf("%d", m == b);
}

int main() {
  constexpr auto ctx = access_context::unchecked();
  constexpr info first = nonstatic_data_members_of(^^S, ctx)[0];
  template for (constexpr info m : define_static_array(nonstatic_data_members_of(^^S, ctx))) {
    printf("%d", m == nonstatic_data_members_of(^^S, ctx)[0]);
    printf("%d", m == first);
    printf("%d", first == m);
  }
  f<int>();
}
