// [meta.reflection] P2996R13: std::meta::info converts to and from
// nothing, has no relational or unary operators, and is consteval-only
// (not trivially copyable in kiln, so a bit cast cannot forge one).
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 19:6 error "cannot initialize a variable of type 'bool' with an rvalue of type 'std::meta::info'"
//@ diag: 20:5 error "cannot initialize a variable of type 'int'"
//@ diag: 21:10 error "C-style cast from 'std::meta::info' to 'long' is not allowed"
//@ diag: 22:6 error "with an rvalue of type 'int'"
//@ diag: 23:6 error "with an rvalue of type 'std::nullptr_t'"
//@ diag: 24:10 error "invalid argument type 'std::meta::info' to unary expression"
//@ diag: 25:16 error "invalid operands to binary expression ('std::meta::info' and 'std::meta::info')"
//@ diag: 26:10 error "destination type must be trivially copyable"
//@ diag: 27:16 error "value of type 'std::meta::info' is not contextually convertible to 'bool'"
//@ diag: 27:16 error "consteval-only expressions are only allowed in a constant-evaluated context"
using info = decltype(^^int);
bool b = ^^int;
int i = ^^int;
long l = (long)^^int;
info x = 0;
info y = nullptr;
bool c = !^^int;
bool d = ^^int < ^^int;
info f = __builtin_bit_cast(info, 1L);
void g() { if (^^int) {} }
