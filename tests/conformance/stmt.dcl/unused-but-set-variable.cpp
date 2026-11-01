// [stmt.dcl], [dcl.attr.unused] (WP-217): -Wunused-but-set-variable (clang
// DiagnoseUnusedButSetDecl / RefsMinusAssignments): a local variable whose
// every reference is the target of a discarded-value assignment, compound
// assignment or increment (an expression statement, a comma's left
// operand, a for-increment, an operand cast to void) is diagnosed as its
// scope is left, together with unused variables in location order. Not a
// reference, class, dependent, [[maybe_unused]] or `_` variable, not a
// parenthesized assignment, not a compound assignment or increment of a
// volatile variable; a condition variable referenced only by its
// condition is diagnosed.
//@ args: -Wunused-but-set-variable -Wunused-variable
//@ strict-diags
//@ diag: 76:11 warning "variable 'z' set but not used"
//@ diag: 78:14 warning "variable 'y' set but not used"
//@ diag: 30:7 warning "variable 'a' set but not used"
//@ diag: 32:7 warning "variable 'b' set but not used"
//@ diag: 34:14 warning "variable 's' set but not used"
//@ diag: 36:8 warning "variable 'p' set but not used"
//@ diag: 38:7 warning "variable 'm' set but not used"
//@ diag: 43:7 warning "variable 'dd' set but not used"
//@ diag: 51:7 warning "variable 'i' set but not used"
//@ diag: 62:16 warning "variable 'w2' set but not used"
//@ diag: 75:7 warning "unused variable 'unused'"
//@ diag: 85:7 warning "variable 'x' set but not used"
struct Tr { int v; };
int h();
volatile int gv;

void f() {
  int a;
  a++;
  int b = 1;
  b += 2;
  static int s = 0;
  s = 1;
  int *p;
  p = nullptr;
  int m;
  for (;;) {
    m = 1;
    break;
  }
  int dd = 0;
  dd++, (void)0;
  int vv;
  vv = 1, vv = 2;
  int c = 0;
  (void)(c = 1);
  int e;
  (e = 1);
  int i;
  for (int k = 0; k < 2; i = k++) {
  }
  int up = 0;
  up = up + 1;
  int &r = up;
  r = 2;
  int q = 0;
  q = sizeof(q);
  volatile int w = 0;
  w += 1;
  volatile int w2;
  w2 = 1;
  Tr t;
  t = Tr{};
  int arr[2];
  arr[0] = 1;
  [[maybe_unused]] int mu;
  mu = 1;
  int _;
  _ = 1;
  int used = 0;
  used = 3;
  gv = used;
  int unused;
  if (int z = h()) {
  }
  while (int y = h())
    y = 0;
  if (int z2 = h())
    gv = z2;
}

template <class T> void tf(T t) {
  int x;
  x = 1;
  T dep;
  dep = t;
}
template void tf<int>(int);
