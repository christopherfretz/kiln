// [expr.const]/10.9: a read of a volatile heap object or constexpr local
// through a glvalue that drops the qualifier is not a core constant
// expression. Every volatile complete object is marked in the Oven.
//@ std: c++26
//@ expect: compile-fail
//@ diag: 19:15 error "constexpr variable 'k' must be initialized by a constant expression"
//@ diag: 15:11 note "read of volatile temporary is not allowed in a constant expression"
//@ diag: 19:19 note "in call to 'f()'"
//@ diag: 24:15 error "constexpr variable 'm' must be initialized by a constant expression"
//@ diag: 22:10 note "read of volatile object 'v' is not allowed in a constant expression"
//@ diag: 24:19 note "in call to 'g()'"
//@ diag: 21:26 note "volatile object declared here"
constexpr int f() {
  volatile int *p = new volatile int(1);
  int r = *const_cast<int *>(p);
  delete p;
  return r;
}
constexpr int k = f();
constexpr int g() {
  constexpr volatile int v = 2;
  return const_cast<const int &>(v);
}
constexpr int m = g();
