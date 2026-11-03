//@ expect: compile-ok
//@ strict-diags
//@ diag: 32:37 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 32:37 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 41:18 warning "overflow in expression; result is 0 with type 'int'"
//@ diag: 34:65 warning "overflow in expression; result is -2 with type 'int'"
//@ diag: 35:35 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 32:37 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 32:37 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 36:44 warning "overflow in expression; result is 0 with type 'int'"
//@ diag: 32:37 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 48:22 warning "overflow in expression; result is 0 with type 'int'"
//@ diag: 39:53 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 32:37 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 32:37 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 32:37 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 57:46 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 32:37 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
// [expr.pre]/4: a signed arithmetic result that is not representable is
// undefined. clang's -Winteger-overflow evaluates a full-expression that
// is not a constant-expression (`EvaluateForOverflow`): a call that is an
// operand is evaluated with the body of the `constexpr` function it
// calls, and each overflowing `+`, `-`, `*` or unary `-` there is
// reported at its operator, once per evaluation (a function called twice
// warns twice), the evaluation going on with the wrapped value. A call
// that is the whole full-expression (or an initializer) is not evaluated,
// only its arguments; a non-constexpr function is not entered. `++` and
// `--` never warn (clang notes them only in a constant expression). In a
// template the warning carries no instantiation notes: a non-dependent
// one is reported for the definition and again for the instantiation.
constexpr int g = 2147483647;
constexpr int inc(int v) { return v + 1; }
int nc(int v) { return v + 1; }
constexpr int loop() { int s = 0; for (int i = 0; i < 3; ++i) s += g; return s; }
constexpr int neg(int v) { return -v; }
constexpr int twice(int v) { return inc(v) + inc(v); }
constexpr int pre() { int i = g; ++i; return i; }
constexpr int post() { int i = -g - 1; i--; return i; }
template <class T> constexpr T tinc(T v) { return v + 1; }
void m() {
  int a = inc(g) + inc(g);
  int b = nc(g) + 1;
  int c = loop() + 0;
  int d = neg(-g - 1) + 0;
  int e = twice(g) + 0;
  int f = inc(g);
  int h = (inc(g));
  int i = 0 + inc(g) * 2;
  int j = inc(inc(g));
  int k = pre() + post();
  int l = tinc(g) + 0;
  (void)a; (void)b; (void)c; (void)d; (void)e; (void)f; (void)h; (void)i; (void)j;
  (void)k; (void)l;
  f = inc(g);
  f += inc(g);
}
template <class T> T tf(T v) { return v + (g + 1); }
int w = tf(1);
template <class T> int tg() { return inc(g) + 0; }
int w2 = tg<int>();
