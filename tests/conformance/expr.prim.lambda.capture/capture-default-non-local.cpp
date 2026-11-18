// [expr.prim.lambda.capture]/3: a lambda-expression shall have no
// capture-default unless its innermost enclosing scope is a block scope,
// or it is in a default member initializer and its innermost enclosing
// scope is that class scope. Clang and GCC both say "non-local lambda
// expression cannot have a capture-default": at namespace scope, in a
// default argument, a template argument, a requires-clause, a static
// member's initializer and a member's declared type (found running GCC's
// C++ testsuite; WP-1652). The accepted side is
// capture-default-local-ok.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:12 error "non-local lambda expression cannot have a capture-default"
//@ diag: 24:12 error "non-local lambda expression cannot have a capture-default"
//@ diag: 25:12 error "non-local lambda expression cannot have a capture-default"
//@ diag: 26:17 error "non-local lambda expression cannot have a capture-default"
//@ diag: 27:20 error "non-local lambda expression cannot have a capture-default"
//@ diag: 28:30 error "non-local lambda expression cannot have a capture-default"
//@ diag: 30:26 error "non-local lambda expression cannot have a capture-default"
//@ diag: 31:29 error "non-local lambda expression cannot have a capture-default"
//@ diag: 32:20 error "non-local lambda expression cannot have a capture-default"
//@ diag: 33:13 error "non-local lambda expression cannot have a capture-default"
//@ diag: 35:52 error "non-local lambda expression cannot have a capture-default"
auto r1 = [&] { return 1; };
auto r2 = [=] { return 1; };
auto r3 = [&, n = 1] { return n; };
int f1(int a = [=] { return 3; }());
template <int N = [&] { return 1; }()> struct T1 {};
template <class T> requires([=] { return true; }()) void h(T) {}
struct S {
  static inline int s = [&] { return 5; }();
  static constexpr int c = [=] { return 6; }();
  void mf(int a = [=] { return 7; }());
  decltype([&] { return 1; }) dm;
};
template <class T> struct U { static inline T s = [=] { return T(); }(); };
