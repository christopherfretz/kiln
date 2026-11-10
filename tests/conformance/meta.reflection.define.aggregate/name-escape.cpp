// [meta.reflection.define.aggregate]/5 (P2996R13), [lex.charset]: the
// name of a data_member_spec, or of the `__kiln_define_member` spike,
// that is not an identifier is quoted in the std::meta::exception's
// message, escaped as clang escapes a string argument (spec/diag.md
// §6.2): a control character as <U+XXXX>, a byte that is not part of
// valid UTF-8 as <XX> (WP-1259: kiln printed the control character raw
// and the byte as U+FFFD). GCC 16.1 does not quote the name ("name is
// not a valid identifier").
//@ expect: compile-fail
//@ args: -Wno-kiln-extension -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 23:35 note "'a<U+0001>b' is not a valid identifier"
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:35 note "'e<FF>f' is not a valid identifier"
//@ diag: 26:17 error "static assertion expression is not an integral constant expression"
//@ diag: 26:17 note "'g<U+0001>h' is not an identifier"
//@ diag: 27:17 error "static assertion expression is not an integral constant expression"
//@ diag: 27:17 note "'i<FF>j' is not an identifier"
#include "kiln_meta.h"
using namespace std::meta;
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "a\x01" "b"})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "e\xff" "f"})));
struct B {
  static_assert(__kiln_define_member(^^B, ^^int, "g\x01" "h") != info{});
  static_assert(__kiln_define_member(^^B, ^^int, "i\xff" "j") != info{});
};
