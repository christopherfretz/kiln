// [meta.reflection] P2996R13: std::meta::info converts to and from
// nothing, has no relational or unary operators, and is consteval-only
// (not trivially copyable in kiln, so a bit cast cannot forge one).
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 18:6 error "cannot initialize a variable of type 'bool' with an rvalue of type 'std::meta::info'"
//@ diag: 19:5 error "cannot initialize a variable of type 'int'"
//@ diag: 20:10 error "C-style cast from 'std::meta::info' to 'long' is not allowed"
//@ diag: 21:6 error "with an rvalue of type 'int'"
//@ diag: 22:6 error "with an rvalue of type 'std::nullptr_t'"
//@ diag: 23:10 error "invalid argument type 'std::meta::info' to unary expression"
//@ diag: 24:16 error "invalid operands to binary expression ('std::meta::info' and 'std::meta::info')"
//@ diag: 25:10 error "destination type must be trivially copyable"
//@ diag: 26:16 error "value of type 'std::meta::info' is not contextually convertible to 'bool'"
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
