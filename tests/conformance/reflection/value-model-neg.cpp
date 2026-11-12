// [meta.reflection] P2996R13: std::meta::info converts to and from
// nothing and has no relational or unary operators; a non-null reflection
// is a consteval-only value ([expr.const.const]/1, P4101R1), and a
// constant-evaluated bit cast cannot forge one ([bit.cast]/2).
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 21:6 error "cannot initialize a variable of type 'bool' with an rvalue of type 'std::meta::info'"
//@ diag: 22:5 error "cannot initialize a variable of type 'int' with an rvalue of type 'std::meta::info'"
//@ diag: 23:10 error "C-style cast from 'std::meta::info' to 'long' is not allowed"
//@ diag: 24:6 error "cannot initialize a variable of type 'info' (aka 'std::meta::info') with an rvalue of type 'int'"
//@ diag: 25:6 error "cannot initialize a variable of type 'info' (aka 'std::meta::info') with an rvalue of type 'std::nullptr_t'"
//@ diag: 26:10 error "invalid argument type 'std::meta::info' to unary expression"
//@ diag: 27:16 error "invalid operands to binary expression ('std::meta::info' and 'std::meta::info')"
//@ diag: 28:16 error "constexpr variable 'f' must be initialized by a constant expression"
//@ diag: 28:20 note "bit_cast to a type with constexpr-unknown representation ('std::meta::info') is not allowed in a constant expression"
//@ diag: 29:16 error "value of type 'std::meta::info' is not contextually convertible to 'bool'"
//@ diag: 29:16 error "consteval-only value outside an immediate function context"
using info = decltype(^^int);
bool b = ^^int;
int i = ^^int;
long l = (long)^^int;
info x = 0;
info y = nullptr;
bool c = !^^int;
bool d = ^^int < ^^int;
constexpr info f = __builtin_bit_cast(info, 1L);
void g() { if (^^int) {} }
