// [stmt.if]/2, [expr.log.and], [expr.log.or], [expr.cond], [stmt.while]
// (WP-234): -Wunreachable-code-aggressive and a constant condition. The
// dead branch of an `if`, `?:`, `&&` or `||` whose condition is a
// configuration value (a call to a constexpr function, a global or a
// `const` local, `sizeof`, a parenthesized or macro-expanded literal, an
// enumerator whose initializer is one) is not reported; an `if
// constexpr` is never pruned. Otherwise a note at the first bare literal
// of the condition offers the parentheses, once per condition. A loop's
// constant condition prunes outright, without the note, unless it is an
// `&&` or `||`.
//@ args: -Wunreachable-code-aggressive -Wno-constant-logical-operand
//@ strict-diags
//@ diag: 47:20 warning "code will never be executed"
//@ diag: 49:20 warning "code will never be executed"
//@ diag: 54:25 warning "code will never be executed"
//@ diag: 54:18 note "silence by adding parentheses to mark code as explicitly dead"
//@ diag: 55:31 warning "code will never be executed"
//@ diag: 55:28 note "silence by adding parentheses to mark code as explicitly dead"
//@ diag: 56:42 warning "code will never be executed"
//@ diag: 56:28 note "silence by adding parentheses to mark code as explicitly dead"
//@ diag: 57:31 warning "code will never be executed"
//@ diag: 57:23 note "silence by adding parentheses to mark code as explicitly dead"
//@ diag: 58:26 warning "code will never be executed"
//@ diag: 58:22 note "silence by adding parentheses to mark code as explicitly dead"
//@ diag: 59:30 warning "code will never be executed"
//@ diag: 59:18 note "silence by adding parentheses to mark code as explicitly dead"
//@ diag: 61:34 warning "code will never be executed"
//@ diag: 61:31 note "silence by adding parentheses to mark code as explicitly dead"
//@ diag: 61:21 warning "code will never be executed"
//@ diag: 61:18 note "silence by adding parentheses to mark code as explicitly dead"
//@ diag: 62:38 warning "code will never be executed"
//@ diag: 62:18 note "silence by adding parentheses to mark code as explicitly dead"
//@ diag: 63:26 warning "code will never be executed"
//@ diag: 64:28 warning "code will never be executed"
//@ diag: 65:49 warning "code will never be executed"
//@ diag: 66:34 warning "code will never be executed"
//@ diag: 66:31 note "silence by adding parentheses to mark code as explicitly dead"
//@ diag: 69:26 warning "code will never be executed"
int g(int);
constexpr bool cfg() { return false; }
const bool kGlobal = false;
enum E { A, B = (0), C = 0 };
#define ZERO 0
void b1() { if (cfg()) g(1); }
void b2() { if (kGlobal) g(1); }
void b3() { const bool k = false; if (k) g(1); }
void b4() { if (A) g(1); }
void b5() { if (B) g(1); }
void b6() { if (C) g(1); }
void b7() { if (ZERO) g(1); }
void b8() { if ((0)) g(1); }
void b9() { if (sizeof(int) == 2) g(1); }
void b10() { if constexpr (false) g(1); }
void b11() { if (false) g(1); }
void b12(int x) { if (x && 0) g(1); }
void b13(int x) { if (x || 1) g(1); else g(2); }
void b14(int x) { if (0 && x) g(2); }
void b15() { int y = 0 ? g(1) : g(2); (void)y; }
void b16() { if (!0) return; g(1); }
void b17() { if (!(0)) return; g(1); }
void b18() { if (0) g(1); if (0) g(2); }
void b19() { if (1) { g(1); } else { g(2); g(3); } }
void b20() { while (0) { g(1); } }
void b21() { while ((0)) { g(1); } }
void b22() { while (sizeof(int) == 4) { g(1); } g(2); }
void b23(int x) { while (x && 0) g(1); }
void b24() { while (true) { g(1); } }
void b25(int x) { while (1) { if (x) break; } g(1); }
void b26() { for (;;) {} g(1); }
void b27(int x) { switch (1) { case 0: g(0); break; case 1: g(1); } }
