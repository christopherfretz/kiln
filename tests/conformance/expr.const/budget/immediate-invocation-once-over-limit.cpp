// [expr.const]/16, [expr.const]/5.1, [implimits]: one step below the
// minimum immediate-invocation-once.cpp needs: each invocation stops at
// the step limit on both kiln and clang 19, and is reported once, in
// place, in every context whose completion checks fold it (WP-1129).
//@ args: -fconstexpr-steps=104
//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:29 error "call to consteval function 'sum' is not a constant expression"
//@ diag: 34:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 36:29 note "in call to 'sum(100)'"
//@ diag: 37:23 error "call to consteval function 'sum' is not a constant expression"
//@ diag: 34:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 37:23 note "in call to 'sum(100)'"
//@ diag: 38:21 error "call to consteval function 'sum' is not a constant expression"
//@ diag: 34:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 38:21 note "in call to 'sum(100)'"
//@ diag: 39:30 error "call to consteval function 'sum' is not a constant expression"
//@ diag: 34:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 39:30 note "in call to 'sum(100)'"
//@ diag: 40:17 error "call to consteval function 'sum' is not a constant expression"
//@ diag: 41:21 note "in the default initializer of 'x'"
//@ diag: 40:9 note "declared here"
//@ diag: 34:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 40:17 note "in call to 'sum(100)'"
//@ diag: 40:17 error "call to consteval function 'sum' is not a constant expression"
//@ diag: 41:29 note "in the default initializer of 'x'"
//@ diag: 40:9 note "declared here"
//@ diag: 34:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 40:17 note "in call to 'sum(100)'"
consteval int sum(int n) {
  int r = 0;
  for (int i = 0; i < n; ++i)
    r += i;
  return r;
}
int ovf(int x) { return x + sum(100); }
short conv() { return sum(100); }
int cond() { return sum(100) ? 1 : 2; }
bool cmp(int x) { return x < sum(100); }
int def(int x = sum(100)) { return x; }
int uses() { return def() + def(); }
