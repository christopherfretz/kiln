// [enum.udecl]/1 (P2996): the splice-type-specifier of a
// using-enum-declarator designates an enumeration.
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ oracle: gcc
//@ diag: 11:14 error "'S' is not an enumerated type"
struct S {};
void f() {
  using enum [: ^^S :];
}
