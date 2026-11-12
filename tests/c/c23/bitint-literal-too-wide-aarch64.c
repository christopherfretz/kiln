// WP-1331: AArch64's BITINT_MAXWIDTH is 128, so a `wb` literal needing
// more bits (with its sign bit) is too large, as clang 19/22 (unsigned:
// "any integer type"; signed: "any signed integer type").
//@ std: c23
//@ args: --target=aarch64-linux-gnu
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:16 error "integer literal is too large to be represented in any integer type"
//@ diag: 13:16 error "integer literal is too large to be represented in any signed integer type"
//@ skip(gcc): GCC's BITINT_MAXWIDTH on AArch64 is 65535

int a = sizeof(0x1ffffffffffffffffffffffffffffffffuwb);
int b = sizeof(0xffffffffffffffffffffffffffffffffwb);
int c = sizeof(0x7fffffffffffffffffffffffffffffffwb);
