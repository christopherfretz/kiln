// [meta.reflection.names]/3 (P2996R13, P3560R2): identifier_of and
// u8identifier_of of a reflection with no identifier throw
// std::meta::exception; uncaught, the enclosing evaluation is not a
// constant expression and the diagnostic carries what() as a note.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 21:15 error "static assertion expression is not an integral constant expression"
//@ diag: 21:15 note "'^^int' has no identifier"
//@ diag: 22:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:15 note "'^^const S' has no identifier"
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:36 note "'^^S *' has no identifier"
//@ diag: 23:15 note "in call to 'ptr_name()'"
#include "kiln_meta.h"
using namespace std::meta;
struct S {};
consteval auto ptr_name() { return identifier_of(^^S*); }

static_assert(identifier_of(^^int).size() == 3);
static_assert(u8identifier_of(^^const S).size() == 1);
static_assert(ptr_name().size() == 1);

int main() {}
