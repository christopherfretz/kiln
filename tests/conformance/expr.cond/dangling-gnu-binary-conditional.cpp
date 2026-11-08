// [expr.cond] GNU binary conditional `a ?: b` (WP-876): as in clang, the
// dangling walk follows only the false operand; the common operand is an
// opaque value evaluated once and is not walked, so a dangling result
// through it (pid(&k) ?: nullptr, id(1) ?: g) is not diagnosed.
//@ strict-diags
//@ expect: compile-fail
//@ diag: 23:39 warning "address of stack memory associated with parameter 'k' returned"
//@ diag: 25:50 error "returning reference to local temporary object"
//@ diag: 28:37 warning "reference to stack memory associated with parameter 'k' returned"
//@ diag: 29:39 error "returning reference to local temporary object"

int g;
const int &id(const int &x [[clang::lifetimebound]]);
int *pid(int *x [[clang::lifetimebound]]);
void f(int k, int *q) {
  const int &a = k ?: 1;
  const int &b = id(1) ?: g;
  const int &c = g ?: id(2);
  int *d = q ?: &k;
  int *e = pid(&k) ?: q;
  (void)a; (void)b; (void)c; (void)d; (void)e;
}
int *r3(int *q, int k) { return q ?: &k; }
const int &r4(int k) { return k ?: g; }
const int &r6(const int &x) { return id(x) ?: id(7); }
int *r7(int *q, int *p) { return q ?: p; }
int *r8(int k) { return pid(&k) ?: nullptr; }
const int &r10(int k) { return k ?: k; }
const int &r11() { return id(1) ?: id(2); }
