// Constant evaluation of a statement expression needs its last statement to
// be an expression statement, and none of its statements may leave it.
//@ expect: compile-fail
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 16:46 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 32:15 note "in call to 'trailing_null()'"
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:32 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 33:15 note "in call to 'last_decl()'"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:14 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 34:15 note "in call to 'early_return(1)'"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 25:13 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 36:15 note "in call to 'leaves_loop()'"
constexpr int trailing_null() { return ({ 2; ; }); }
constexpr int last_decl() { ({ int x = 1; }); return 0; }
constexpr int early_return(int v) {
  int r = ({ if (v) return 5; 3; });
  return r;
}
constexpr int leaves_loop() {
  int n = 0;
  for (;;) {
    n += ({ if (n > 2) break; 1; });
  }
  return n;
}
constexpr int inner_loop() {
  return ({ int s = 0; for (int i = 0; i < 3; ++i) { if (i == 1) continue; s += i; } s; });
}
static_assert(trailing_null() == 2);
static_assert(last_decl() == 0);
static_assert(early_return(1) == 5);
static_assert(early_return(0) == 3);
static_assert(leaves_loop() == 3);
static_assert(inner_loop() == 2);
