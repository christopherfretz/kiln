// [stmt.return]/1, [stmt.break], [stmt.for]/1 (WP-234): -Wunreachable-code
// covers -Wunreachable-code-loop-increment and -fallthrough but not the
// dead `break` and `return` of -Wunreachable-code-aggressive.
//@ args: -Wunreachable-code
//@ strict-diags
//@ diag: 11:41 warning "loop will run at most once (loop increment never executed)"
//@ diag: 12:21 warning "code will never be executed"
int g(int);
void d1(int x) { while (x) { break; break; } }
int d2(int x) { return x; return g(1); }
void d3(int n) { for (int i = 0; i < n; ++i) { return; } }
void d4() { return; g(1); }
