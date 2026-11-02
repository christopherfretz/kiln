// [dcl.asm] GNU register variables, aarch64: a block-scope one needs a
// register name the target knows ("unknown register name"); a global one
// also one the target can hold a variable in (AArch64: sp, x*, w*; x86_64:
// rsp, rbp, esp, ebp; else "unsuitable for global register variables")
// of the variable's size (x*/sp/rsp/rbp 64 bits, w*/esp/ebp 32: "size of
// register does not match") and an integral or pointer type ("bad type
// for named register variable" at the decl-specifiers). clang 19 checks
// sizes only for globals (WP-327).
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ diag: 35:21 error "size of register 'x0' does not match variable size"
//@ diag: 37:22 error "size of register 'w0' does not match variable size"
//@ diag: 38:21 error "size of register 'sp' does not match variable size"
//@ diag: 41:22 error "register 'lr' unsuitable for global register variables on this target"
//@ diag: 42:22 error "register 'r8' unsuitable for global register variables on this target"
//@ diag: 43:22 error "unknown register name 'rsp' in asm"
//@ diag: 44:21 error "unknown register name 'rsp' in asm"
//@ diag: 45:22 error "unknown register name 'ebp' in asm"
//@ diag: 46:22 error "unknown register name '%rsp' in asm"
//@ diag: 47:22 error "unknown register name 'rax' in asm"
//@ diag: 48:22 error "unknown register name 'bogus' in asm"
//@ diag: 49:22 error "size of register 'w0' does not match variable size"
//@ diag: 52:21 error "unknown register name 'rsp' in asm"
//@ diag: 52:1 error "bad type for named register variable"
//@ diag: 53:23 error "size of register 'x0' does not match variable size"
//@ diag: 53:1 error "bad type for named register variable"
//@ diag: 54:16 error "bad type for named register variable"
//@ diag: 56:14 warning "ignored asm label 'x8' on automatic variable"
//@ diag: 58:24 error "unknown register name 'rax' in asm"
//@ diag: 59:23 error "unknown register name 'nosuchreg' in asm"
//@ requires: clang<22
// clang 22 emits error: register 'x8' unsuitable for global register variables on this target; WP-355 residual
register long a1 asm("x0");
register int a2 asm("x0");
register int a3 asm("w0");
register char a4 asm("w0");
register int a5 asm("sp");
register long a6 asm("x8");
register int a7 asm("wsp");
register long a8 asm("lr");
register long a9 asm("r8");
register long b1 asm("rsp");
register int b2 asm("rsp");
register long b3 asm("ebp");
register long b4 asm("%rsp");
register long b5 asm("rax");
register long b6 asm("bogus");
register bool b7 asm("w0");
register long *b8 asm("x0");
struct Big { long a, b; };
register Big b9 asm("rsp");
register float c1 asm("x0");
enum E { e1 }; register E c2 asm("w0");
void f() {
  int au asm("x8");
  register long l1 asm("x8");
  register char l2 asm("rax");
  register int l3 asm("nosuchreg");
  register Big l4 asm("x0");
}
