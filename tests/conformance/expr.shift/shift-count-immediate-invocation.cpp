// [expr.shift]/1: a shift count that is negative or not less than the
// width of the promoted left operand is undefined; clang warns
// (-Wshift-count-negative, -Wshift-count-overflow) when the operator is
// built (DiagnoseBadShiftValues, EvaluateAsInt). A count that is an
// immediate invocation ([expr.const]/16) is evaluated once there, its
// result reused at the end of the full-expression (WP-1171): in range,
// negative and too large counts, promoted and long left operands, a
// compound assignment, the order against an overflow warning, a failing
// invocation reported once, a count under an operator, file scope and a
// constexpr initializer.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:32 warning "shift count is negative"
//@ diag: 31:33 warning "shift count >= width of type"
//@ diag: 32:32 warning "shift count >= width of type"
//@ diag: 34:34 warning "shift count >= width of type"
//@ diag: 35:25 warning "shift count is negative"
//@ diag: 36:56 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 36:30 warning "shift count >= width of type"
//@ diag: 37:34 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 26:38 note "division by zero"
//@ diag: 37:34 note "in call to 'bad(0)'"
//@ diag: 38:38 warning "shift count is negative"
//@ diag: 39:20 warning "shift count >= width of type"
consteval int cnt(int n) { return n; }
consteval int bad(int x) { return 10 / x; }
consteval short scnt(short n) { return n; }
int in_range(int x) { return x << cnt(3); }
int in_range_paren(int x) { return x >> (cnt(31)); }
int negative(int x) { return x << cnt(-1); }
int too_large(int x) { return x >> cnt(32); }
int promoted(int x) { return x << scnt(40); }
long wide(long x) { return x << cnt(40); }
long too_wide(long x) { return x << cnt(64); }
int compound(int x) { x <<= cnt(-2); return x; }
int order(int x) { return (x << cnt(33)) + (2147483647 + 1); }
int failing(int x) { return x << bad(0); }
int under_operator(int x) { return x << (cnt(2) - 3); }
int file_scope = 1 << cnt(64);
constexpr int folded = 1 << cnt(30);
static_assert(folded == 1073741824);
