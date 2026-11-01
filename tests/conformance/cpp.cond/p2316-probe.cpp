// [cpp.cond]/12 (C++23, P2316R2): character literals in #if use the same
// encoding as in C++ expressions. WP-185 feature probe.
//@ expect: compile-ok
//@ strict-diags
#if 'A' != 0x41 || u8'a' != 0x61
#error character literal encoding in #if differs from the execution encoding
#endif
static_assert('A' == 0x41);
