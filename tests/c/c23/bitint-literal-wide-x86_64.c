// WP-1331: x86-64's BITINT_MAXWIDTH is 8388608, so clang 19/22 type a
// `wb` literal wider than 128 bits as _BitInt(N); kiln's literal values
// are 128-bit and it reports the literal too large.
//@ std: c23
//@ args: --target=x86_64-linux-gnu
//@ xfail(kiln): WP-1331 follow-up, integer literal values are u128, so a wb literal over 128 bits is "too large" instead of _BitInt(N)
//@ skip(gcc): GCC's x86-64 _BitInt literal limits differ

int a = sizeof(0x1ffffffffffffffffffffffffffffffffuwb);
int b = sizeof(0xffffffffffffffffffffffffffffffffwb);
_Static_assert(sizeof(0x1ffffffffffffffffffffffffffffffffuwb) == 24, "");
