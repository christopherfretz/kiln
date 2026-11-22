// [cpp.cond]/13: an identifier left in a #if expression is replaced by
// 0. With -Wundef-prefix=<prefixes>, a live one starting with one of the
// prefixes warns "'X' is not defined, evaluates to 0" (clang
// `warn_pp_undef_prefix`, shown as [-Wundef-prefix=FOO_,BA]); one inside
// `defined(...)`, a defined macro, another name, or an operand of a
// short-circuited operator does not. WP-1816. Verified against clang 22
// and 19.
//@ args: -Wundef-prefix=FOO_,BA
//@ strict-diags
//@ diag: 16:5 warning "'FOO_X' is not defined, evaluates to 0"
//@ diag: 18:5 warning "'BAR' is not defined, evaluates to 0"
//@ diag: 20:23 warning "'FOO_Z' is not defined, evaluates to 0"
#define FOO_DEFINED 1
#if FOO_DEFINED
#endif
#if FOO_X
#endif
#if BAR
#endif
#if defined(FOO_Y) || FOO_Z
#endif
#if OTHER || 1 || FOO_W
#endif
#if 0 && BAZ
#endif
int main() {}
