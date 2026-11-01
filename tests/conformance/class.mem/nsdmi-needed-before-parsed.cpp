// [class.mem.general]/8: a default member initializer of a nested class is
// parsed at the end of the outermost class; needing it earlier (aggregate
// initialization in a static member initializer) is an error, and the
// field is invalid afterwards (later uses, default arguments, are quiet).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:37 error "default member initializer for 'n' needed within definition of enclosing class 'S' outside of member functions"
//@ diag: 12:18 note "default member initializer declared here"
//@ diag: 18:38 error "default member initializer for 'a' needed within definition of enclosing class 'W2' outside of member functions"
//@ diag: 17:18 note "default member initializer declared here"
struct S {
  struct T { int n = 0; };
  void f(T t = T());
  static constexpr int k = sizeof(T{});
};
struct W2 {
  struct X { int a = 2; };
  static constexpr int sz = sizeof(X{});
  static const int q = X().a;
};
constexpr int k = W2::X().a;
