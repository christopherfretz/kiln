// [temp.friend]/1, [class.friend]/2: `friend R T::f(...);` with `T` a
// template parameter befriends, in each specialization, the member
// function the substituted qualifier names: a member function with a
// dependent parameter type, a const member operator, a member of a nested
// class, a static member function; no other member gets access (WP-1495;
// found running GCC's C++ testsuite).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 34:28 error "'secret' is a private member of 'S<P>'"
//@ diag: 17:7 note "implicitly declared private here"
template<class T> class S {
  friend int T::f(typename T::type);
  friend bool T::operator==(const T &) const;
  friend int T::N::g();
  friend int T::st();
  int secret = 1;
public:
  S() {}
};
struct P {
  typedef long type;
  struct N { int g(); };
  int f(type);
  bool operator==(const P &) const;
  static int st();
  int h();
};
int P::f(type) { return S<P>().secret; }
bool P::operator==(const P &) const { return S<P>().secret; }
int P::N::g() { return S<P>().secret; }
int P::st() { return S<P>().secret; }
// Not befriended.
int P::h() { return S<P>().secret; }
