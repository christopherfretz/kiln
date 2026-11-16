// C++14 [dcl.constexpr] (N3652): a constexpr member function is no longer
// implicitly const; the C++11 sibling constexpr-not-const-cxx11 warns
// about the change. Calls on a const object need a written const.
//@ std: c++14
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:30 error "'this' argument to member function 'f' has type 'const S', but function is not marked const"
//@ diag: 15:17 note "'f' declared here"
//@ diag: 30:38 error "'this' argument to member function 'k' has type 'const S', but function is not marked const"
//@ diag: 26:18 note "'k' declared here"
//@ diag: 30:48 error "no matching member function for call to 't'"
//@ diag: 20:36 note "candidate function template not viable: 'this' argument has type 'const S', but method is not marked const"
struct S {
  int x;
  constexpr int f() { return x; }
  constexpr int g() const { return x; }
  constexpr S(int a) : x(a) {}
  static constexpr int h() { return 1; }
  static constexpr int h2();
  template <class T> constexpr int t(T) { return x; }
  constexpr int k();
  constexpr int &r() & { return x; }
  int m();
  constexpr int callm() { return m(); }
};
constexpr int S::k() { return x; }
constexpr int S::h2() { return 2; }
template <class T> struct U { constexpr int f() { return 0; } };
U<int> u;
int use(const S &s) { return s.f() + s.k() + s.t(1) + u.f(); }
struct V {
  constexpr int f();
  int f() const;
};
