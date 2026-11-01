// [stmt.for] (WP-234): -Wloop-analysis enables -Wfor-loop-analysis (and
// -Wrange-loop-analysis).
//@ args: -Wloop-analysis
//@ strict-diags
//@ diag: 10:10 warning "variables 'i' and 'n' used in loop condition not modified in loop body"
//@ diag: 11:35 warning "variable 'j' is incremented both in the loop header and in the loop body"
//@ diag: 11:28 note "incremented here"
void f(int n) {
  int i = 0;
  for (; i < n; ) {}
  for (int j = 0; j < n; ++j) { ++j; }
}
