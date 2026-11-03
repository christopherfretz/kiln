// [depr.volatile.type] (C++20): an increment or decrement of a volatile
// object (at the operator), the use of the result of a simple assignment
// to one (at its left operand; not when the assignment is a
// discarded-value expression, ignoring parentheses and implicit
// conversions, nor in an unevaluated operand; compound assignment is not
// warned, as in clang) and a volatile structured binding declaration (at
// `volatile`) are deprecated (-Wdeprecated-volatile, clang
// `warn_deprecated_increment_decrement_volatile`,
// `warn_deprecated_simple_assign_volatile`,
// `warn_deprecated_volatile_structured_binding`). Same at clang 19 and 22
// (WP-432).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
//@ diag: 48:15 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 49:14 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 50:23 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 58:12 warning "use of result of assignment to object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 59:12 warning "use of result of assignment to object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 65:25 warning "use of result of assignment to object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 66:4 warning "increment of object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 67:3 warning "decrement of object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 68:7 warning "increment of object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 69:3 warning "increment of object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 70:5 warning "decrement of object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 71:7 warning "increment of object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 60:5 warning "use of result of assignment to object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 61:8 warning "use of result of assignment to object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 62:15 warning "use of result of assignment to object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 63:7 warning "use of result of assignment to object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 64:7 warning "use of result of assignment to object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 64:15 warning "use of result of assignment to object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 77:11 warning "use of result of assignment to object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 79:3 warning "volatile qualifier in structured binding declaration is deprecated"
//@ diag: 80:9 warning "volatile qualifier in structured binding declaration is deprecated"
//@ diag: 81:8 warning "volatile qualifier in structured binding declaration is deprecated"
//@ diag: 83:10 warning "volatile qualifier in structured binding declaration is deprecated"
//@ diag: 85:36 warning "increment of object of volatile-qualified type 'volatile int' is deprecated"
//@ diag: 86:15 note "in instantiation of function template specialization 't<volatile int>' requested here"
//@ diag: 85:44 warning "use of result of assignment to object of volatile-qualified type 'volatile int' is deprecated"
volatile int gv;
int f(int);
struct C { int a, b; };
void g(volatile int &r, volatile int *p, volatile C &vc) {
  r += 1;
  r |= 1;
  *p *= 2;
  (void)sizeof(r = 1);
  decltype(r = 1) q = r;
  bool b = noexcept(r = 1);
  (void)(r = 1);
  static_cast<void>(r = 9);
  (r = 1);
  ((r = 1));
  for (;; r = 1) break;
  r = 1 ? 2 : 3;
  vc.a = 3;
  int z = (r = 1);
  int w = (vc.a = 4);
  f(r = 2);
  if ((r = 5)) {}
  (void)(gv = gv = 1);
  r = r = 1;
  0 ? r = 1 : r = 2;
  auto l = [&] { return r = 7; };
  r++;
  --r;
  p[0]++;
  ++*p;
  gv--;
  vc.a++;
  int i = 0;
  i = r;
  i++;
  (void)q; (void)b; (void)z; (void)w; (void)l;
}
int ns = (gv = 1);
void h() {
  volatile auto [a0, b0] = C{1, 2};
  const volatile auto [a1, b1] = C{1, 2};
  auto volatile [a2, b2] = C{1, 2};
  const auto &[a3, b3] = C{1, 2};
  static volatile auto [a4, b4] = C{1, 2};
}
template <class T> void t(T &x) { x++; x = x = 1; }
template void t(volatile int &);
template void t(int &);
