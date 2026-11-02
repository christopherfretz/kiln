// [dcl.asm], GNU asm qualifiers and operands: duplicate and ignored
// qualifiers, a non-literal template, tied operands of incompatible types,
// duplicate matching constraints, a member pointer through a register, and
// x86 clobber names; namespace-scope asm takes an ordinary literal.
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 22:16 error "duplicate asm qualifier 'volatile'"
//@ diag: 23:7 error "expected 'volatile', 'inline', 'goto', or '('"
//@ diag: 24:7 error "expected string literal in 'asm'"
//@ diag: 25:26 error "unsupported inline asm: input with type 'S' matching output with type 'int'"
//@ diag: 27:43 error "more than one input constraint matches the same output '0'"
//@ diag: 27:35 note "constraint '0' is already present here"
//@ diag: 28:17 error "cannot pass a pointer-to-member through register-constrained inline assembly parameter"
//@ diag: 29:69 error "unknown register name 'bogus' in asm"
//@ diag: 32:12 error "cannot use wide string literal in 'asm'"
//@ requires: clang<22
// clang 22 emits error: expected string literal or parenthesized constant expression in 'asm'; WP-355 residual
struct S { int a[3]; };
struct M { void f(); };
void g(int x, double d, S s) {
  asm volatile volatile("");
  asm const("");
  asm(1);
  asm("" : "=r"(x) : "0"(s));
  int y;
  asm("" : "=r"(x), "=r"(y) : "0"(x), "0"(y));
  asm("" :: "r"(&M::f));
  asm("" : "=r"(d) : "r"(x) : "rax", "eax", "cc", "memory", "xmm0", "bogus");
}
asm("ok" "concat");
asm(L"wide");
