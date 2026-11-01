// [temp.dep.constexpr]/2.3: an id-expression naming a static member function
// that is a dependent member of the current instantiation is value-dependent,
// so a condition calling it waits for instantiation (libc++ `extents`:
// `if constexpr (rank() > 0)`, where `rank()` returns `__rank_ =
// sizeof...(_Extents)`). A call to a function that is not a member of the
// current instantiation is checked at the definition.
//@ expect: compile-fail
//@ diag: 21:28 error "constexpr if condition is not a constant expression"
//@ diag: 22:28 error "constexpr if condition is not a constant expression"
//@ diag: 30:28 error "static assertion failed due to requirement 'r() == 2'"
//@ diag: 32:17 note "in instantiation of member function 'S<1>::f' requested here"
int h();
constexpr int nc() { return h(); }
template <class T> struct Y { static constexpr int m() { return nc(); } };
template <class T, int... E> struct X {
  static constexpr int r_ = sizeof...(E);
  static constexpr int rank() noexcept { return r_; }
  static constexpr int bad() { return nc(); }
  template <class U> constexpr X(U) { if constexpr (rank() > 0) { } }
  void g() { if constexpr (X::rank() > 0) { } static_assert(rank() >= 0); }
  void a() { if constexpr (nc() > 0) { } }
  void b() { if constexpr (Y<int>::m() > 0) { } }
  void c() { if constexpr (bad() > 0) { } }
  static constexpr int z() { return rank() * 2; }
};
X<int, 1, 2> x(1);
static_assert(X<int, 1, 2, 3>::z() == 6);
template <int... E> struct S {
  static constexpr int r() { return sizeof...(E); }
  void f() { static_assert(r() == 2); }
};
template struct S<1>;
