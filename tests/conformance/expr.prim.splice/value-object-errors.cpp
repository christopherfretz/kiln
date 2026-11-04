// [expr.prim.splice]/2 (P2996R13): a value splice is a prvalue, so it is
// not assignable and has no address ([expr.assign]/1, [expr.unary.op]/3);
// an object splice is an lvalue of the object's type, so a template
// parameter object (const, [temp.param]/8) and a constexpr variable are
// not modifiable through it. reflect_object of an object without static
// storage duration is not a constant expression
// ([meta.reflection.result]: the argument must be suitable for use as a
// constant template argument). GCC 16.1 rejects each line too; its wording
// differs ("lvalue required as left operand of assignment", "assignment of
// read-only location", "reflect_object failed"; spec/reflection.md §5).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 33:13 error "expression is not assignable"
//@ diag: 34:7 error "cannot take the address of an rvalue of type 'int'"
//@ diag: 35:19 error "read-only variable is not assignable"
//@ diag: 36:14 error "cannot assign to variable 'limit' with const-qualified type 'const int'"
//@ diag: 27:15 note "variable 'limit' declared const here"
//@ diag: 40:12 error "splice operand must be a constant expression"
//@ diag: 40:12 note "pointer to 'local' is not a constant expression"
//@ diag: 39:7 note "declared here"
//@ diag: 42:11 error "call to consteval function 'read_local' is not a constant expression"
#include <meta>
struct Chars { char c[4]{}; };
constexpr int limit = 3;
constexpr auto rfive = std::meta::reflect_constant(5);
constexpr auto rchars = std::meta::reflect_constant(Chars{{'a'}});
constexpr auto rlimit = std::meta::reflect_object(limit);
void misuse() {
  int* p;
  [:rfive:] = 4;
  p = &[:rfive:];
  [:rchars:].c[0] = 'b';
  [:rlimit:] = 2;
}
consteval int read_local() {
  int local = 1;
  return [:std::meta::reflect_object(local):];
}
int use = read_local();
