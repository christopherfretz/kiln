// [class.mem.general]/7: a default argument of a member function is a
// complete-class context: it names members declared later, and is
// available once the class is complete (in member function bodies too).
// Using it before then (outside a complete-class context) is an error
// ([dcl.fct.default]; clang "use of default argument to function ... that
// is declared later in class").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:17 error "use of default argument to function 'f' that is declared later in class 'S'"
//@ diag: 14:34 note "default argument declared here"
//@ diag: 19:11 error "use of default argument to function 'h' that is declared later in class 'S'"
//@ diag: 17:34 note "default argument declared here"
struct S {
  static constexpr int f(int a = k) { return a; }
  static constexpr int f(int, int) { return 0; }
  constexpr int g(int a = k + 1) const { return a; }
  static constexpr int h(int a = k);
  static_assert(f() == 1);
  int arr[h()];
  constexpr int body() const { return f() + g(); }
  static constexpr int k = 1;
};
constexpr int S::h(int a) { return a; }
static_assert(S::f() == 1 && S{}.g() == 2 && S::h() == 1 && S{}.body() == 3);

template <class T> struct TS {
  static constexpr T f(T a = k) { return a; }
  static constexpr T k = 2;
};
static_assert(TS<int>::f() == 2);
