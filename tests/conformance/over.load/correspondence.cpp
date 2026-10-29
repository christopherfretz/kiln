// [over.load]/2, [basic.scope.scope]/4, [temp.over.link]: declarations that
// do not correspond declare overloads; corresponding ones redeclare.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:8 error "static and non-static member functions with the same parameter types cannot be overloaded"
//@ diag: 22:15 note "previous declaration is here"
//@ diag: 25:8 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 24:7 note "previous declaration is here"
//@ diag: 27:8 error "class member cannot be redeclared"
//@ diag: 26:8 note "previous declaration is here"
//@ diag: 43:5 error "redefinition of 'v' as different kind of symbol"
//@ diag: 42:5 note "previous definition is here"
void f(int);
void f(double);
void f(int);
void f(int) {}
struct S {
  void m();
  void m() const;
  void m(int) &&;
  void m(int) const &&;
  static void s(int);
  void s(int);
  int r();
  long r();
  void q(char);
  void q(char);
};
void S::m() {}
void S::m() const {}
template<class T> T g(T);
template<class T> int g(T);
template<class T> T g(T);
template<class T> void h(T) requires (sizeof(T) > 1);
template<class T> void h(T) requires (sizeof(T) > 4);
template<class T> void h(T) requires (sizeof(T) > 1);
template<class T, class U> void k(T, U);
template<class U, class T> void k(T, U);
template<int N> void n(int (&)[N]);
template<long N> void n(int (&)[N]);
namespace X { void v(); int v(int); }
int v;
int v(int);
