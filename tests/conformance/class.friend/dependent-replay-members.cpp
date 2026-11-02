// [temp.friend], [class.access]: in a class template befriended only as X<int>,
// access in member functions, member templates, lambdas and generic lambdas is
// decided per instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:43 error "'x' is a private member of 'A'"
//@ diag: 39:19 note "in instantiation of function template specialization 'X<char>::m<int>' requested here"
//@ diag: 25:7 note "implicitly declared private here"
//@ diag: 33:31 error "'x' is a private member of 'A'"
//@ diag: 40:18 note "in instantiation of member function 'X<char>::s' requested here"
//@ diag: 25:7 note "implicitly declared private here"
//@ diag: 34:36 error "'x' is a private member of 'A'"
//@ diag: 41:19 note "in instantiation of member function 'X<char>::l' requested here"
//@ diag: 25:7 note "implicitly declared private here"
//@ diag: 35:43 error "'x' is a private member of 'A'"
//@ diag: 35:21 note "in instantiation of function template specialization 'X<char>::gl()::(lambda)::operator()<int>' requested here"
//@ diag: 42:19 note "in instantiation of member function 'X<char>::gl' requested here"
//@ diag: 25:7 note "implicitly declared private here"
//@ diag: 36:23 error "'N' is a private member of 'A'"
//@ diag: 43:18 note "in instantiation of member function 'X<char>::n' requested here"
//@ diag: 26:10 note "implicitly declared private here"
//@ requires: clang>=22
template<class T> struct X;
class A {
  int x;
  struct N { };
  friend struct X<int>;
public:
  A();
};
template<class T> struct X {
  template<class U> int m(U) { return A().x; }
  static int s() { return A().x; }
  int l() { return [] { return A().x; }(); }
  int gl() { return [](auto) { return A().x; }(1); }
  static int n() { A::N v; (void)v; return 0; }
};
int a = X<int>().m(1) + X<int>::s() + X<int>().l() + X<int>().gl() + X<int>::n();
int b = X<char>().m(1);
int c = X<char>::s();
int d = X<char>().l();
int e = X<char>().gl();
int f = X<char>::n();
