// [stmt.break], [stmt.cont], [stmt.for]/1, [stmt.ranged], [stmt.switch],
// [dcl.attr.fallthrough] (WP-234): -Wunreachable-code-aggressive reports
// a dead `break` as "'break' will never be executed" and a dead loop
// increment as "loop will run at most once" (at the increment, or the
// `for` of a range-based loop). The literal or enumerator condition of
// a `do` loop that never repeats is not reported, every case label of a
// reached switch is reached, and a dead `[[fallthrough]];` gets only
// -Wunreachable-code-fallthrough.
//@ args: -Wunreachable-code-aggressive
//@ strict-diags
//@ diag: 24:37 warning "code will never be executed"
//@ diag: 25:47 warning "'break' will never be executed"
//@ diag: 26:46 warning "code will never be executed"
//@ diag: 27:31 warning "code will never be executed"
//@ diag: 28:41 warning "loop will run at most once (loop increment never executed)"
//@ diag: 29:24 warning "loop will run at most once (loop increment never executed)"
//@ diag: 30:41 warning "loop will run at most once (loop increment never executed)"
//@ diag: 33:42 warning "loop will run at most once (loop increment never executed)"
//@ diag: 37:40 warning "code will never be executed"
//@ diag: 41:19 warning "fallthrough annotation in unreachable code"
int g(int);
[[noreturn]] void die();
enum E { A };
void c1(int x) { while (x) { break; g(1); } }
void c2(int x) { switch (x) { case 0: return; break; default: g(1); } }
void c3(int x) { switch (x) { case 1: break; g(1); break; } }
void c4(int x) { switch (x) { g(0); case 1: break; } }
void c5(int n) { for (int i = 0; i < n; ++i) { return; } }
void c6(int (&a)[2]) { for (int v : a) { (void)v; return; } }
void c7(int n) { for (int i = 0; i < n; ++i) { break; } }
void c8(int n) { for (int i = 0; i < n; ++i) { continue; } }
void c9(int n) { for (int i = 0; i < n; ++i) { if (n) continue; else break; } }
void c10(int n) { for (int i = 0; i < 3; ++i) { die(); } }
void c11() { do { return; } while (0); }
void c12() { do { return; } while (A); }
void c13(int x) { do { g(1); continue; } while (x); }
void c14(int x) { do { break; } while (g(1)); }
void c15(int x) {
  switch (x) {
  case 1: g(1); [[fallthrough]];
  case 2: return; [[fallthrough]];
  case 3: g(3);
  }
}
