// [dcl.init.string]/1 (C++20 DR, P2513R4): an array of `char` or
// `unsigned char` may be initialized by a UTF-8 string literal.
// WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
constexpr char a[] = u8"abc";
constexpr unsigned char b[] = u8"abc";
static_assert(sizeof(a) == 4 && sizeof(b) == 4);
static_assert(a[1] == 'b');
