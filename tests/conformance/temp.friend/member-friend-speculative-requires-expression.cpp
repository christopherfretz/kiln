// [temp.friend]/5, [expr.prim.req.general]/5, [temp.res.general]/6: the
// friend `template <class T> template <class U> friend int A<T>::b(U,
// const B &, int);` names no member of any A<T> (A<T>::b has two
// parameters), while the friend of A<T>::c names A<T>::c. In the pattern
// both are granted speculatively, so neither decides a requires-expression
// there: A<int>::b<int> evaluates its requirements as false and A<int>::c
// as true, with no hard error. (A non-dependent `x.n` outside the
// requires-expression in b, even in a discarded statement, would be
// checked again in A<int>::b<int> as a hard error, as clang does for the
// befriended-specialization case: `expr.prim.req.general/
// speculative-access-denied.cpp`.)
//@ skip(clang): clang does not model [temp.friend]/5 friends (-Wunsupported-friend)
//@ expect: run
//@ stdout: 0 0 0 1 1 1
//@ strict-diags
//@ diag: 25:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 26:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
extern "C" int printf(const char *, ...);
class B;
template <class T> struct A {
  template <class U> static int b(U, const B &, int *);
  template <class U> static int c(U, const B &, int *);
};
class B {
  template <class T> template <class U> friend int A<T>::b(U, const B &, int);
  template <class T> template <class U> friend int A<T>::c(U, const B &, int *);
  int n = 5;
  static constexpr int sn = 5;
public:
  B() {}
};
template <class T> template <class U> int A<T>::b(U, const B &x, int *r) {
  r[0] = requires { x.n; };
  r[1] = requires { B::sn; };
  if constexpr (requires { B().n; }) r[2] = 1;
  else r[2] = 0;
  return 0;
}
template <class T> template <class U> int A<T>::c(U, const B &x, int *r) {
  r[0] = requires { x.n; };
  r[1] = requires { B::sn; };
  if constexpr (requires { B().n; }) r[2] = 1;
  else r[2] = 0;
  return 0;
}
int main() {
  B b;
  int r[6];
  A<int>::b(1, b, r);
  A<int>::c(1, b, r + 3);
  printf("%d %d %d %d %d %d\n", r[0], r[1], r[2], r[3], r[4], r[5]);
}
