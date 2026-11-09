// [lex.operators], [cpp.predefined] P2996R13: below C++26 reflection stays
// off (operator decision Q7, WP-920) and -freflection does not change that:
// the flag is accepted, `^^` still lexes as two `^` (the reflection tokens
// need C++26), and `__cpp_impl_reflection` stays undefined.
//@ std: c++23
//@ args: -freflection
//@ skip(clang): -freflection is not a clang 19/22 flag
//@ expect: compile-fail
//@ diag: 13 error "expected expression"
#ifdef __cpp_impl_reflection
#error "__cpp_impl_reflection below C++26"
#endif
int a = ^^int;
