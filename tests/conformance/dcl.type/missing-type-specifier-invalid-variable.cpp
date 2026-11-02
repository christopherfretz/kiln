// [dcl.type.general]/2, [dcl.init.general]/7: a variable whose
// decl-specifier-seq names no type (`const u7;`: the undeclared name is the
// declarator-id) is invalid after "a type specifier is required for all
// declarations", so its missing initializer is not checked: no "default
// initialization of an object of const type" for a const one (clang's
// `ConvertDeclSpecToType` marks the declarator invalid).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:7 error "a type specifier is required for all declarations"
//@ diag: 17:9 error "a type specifier is required for all declarations"
//@ diag: 18:16 error "a type specifier is required for all declarations"
//@ diag: 19:9 error "a type specifier is required for all declarations"
//@ diag: 20:13 error "a type specifier is required for all declarations"
//@ diag: 23:18 error "a type specifier is required for all declarations"
const u1;
void f() {
  const u7;
  static const u8;
  const u9 = 1, u10;
  constexpr u11 = 2;
  int i = u7 + 1;
}
struct A { const u12; };
