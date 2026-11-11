// C23 6.7.11p4: every expression in an initializer of an object with static
// storage duration is a constant expression (clang
// `CheckForConstantInitializer`): a call, a parameter's value, the address of
// an automatic object and an address cast to an integer narrower than a
// pointer are not (6.6p9-p10), at file scope and for a block-scope static.
// The first element of a list that is not a constant is reported. Such an
// object is never initialized at run time in C.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:9 error "initializer element is not a compile-time constant"
//@ diag: 17:20 error "initializer element is not a compile-time constant"
//@ diag: 18:11 error "initializer element is not a compile-time constant"
//@ diag: 20:18 error "initializer element is not a compile-time constant"
//@ diag: 21:19 error "initializer element is not a compile-time constant"
int g; int h(void);
int a = h();
int b[3] = { 1, 2, h() };
short c = (short)(long)&g;
void f(int x) {
  static int s = x;
  static int *q = &x;
  (void)s; (void)q;
}
