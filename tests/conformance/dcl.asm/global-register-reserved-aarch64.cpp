// [dcl.asm] GNU global register variables, aarch64, clang 22 rule: an x/w
// register must also be reserved, and on a Linux target only register 0 is
// (x18 is reserved by default on other OSes, the rest by +reserve-xN);
// sp stays allowed. clang 19 accepted every x/w register.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu
//@ requires: clang>=22
//@ strict-diags
//@ diag: 17:22 error "register 'x8' unsuitable for global register variables on this target"
//@ diag: 18:21 error "register 'wsp' unsuitable for global register variables on this target"
//@ diag: 19:21 error "register 'w18' unsuitable for global register variables on this target"
//@ diag: 20:22 error "register 'x30' unsuitable for global register variables on this target"
// kiln at -fclang-compat=22 (WP-377).
register long r0 asm("x0");
register int r1 asm("w0");
register long r2 asm("sp");
register long r3 asm("x8");
register int r4 asm("wsp");
register int r5 asm("w18");
register long r6 asm("x30");
