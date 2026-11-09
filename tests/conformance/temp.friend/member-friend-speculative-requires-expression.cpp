// [temp.friend]/5, [expr.prim.req.general]/5, [temp.res.general]/6: the
// friend `template <class T> template <class U> friend int A<T>::b(U,
// const B &, int);` names no member of any A<T> (A<T>::b has two
// parameters), so in A<int>::b the non-dependent `requires { x.n; }` is
// false and the call returns 0. kiln grants the pattern's access
// speculatively, takes the true branch, then replays the check in
// A<int>::b<int> as a hard error ('n' is a private member of 'B'). clang
// turns off access control for B (-Wunsupported-friend) and returns 5;
// both values pass here, the rejection is the bug.
//@ xfail(kiln): WP-1145 follow-up, a speculative [temp.friend]/5 grant decides a non-dependent requires-expression in the pattern and is replayed as a hard error
//@ expect: run
class B;
template <class T> struct A {
  template <class U> static int b(U, const B &);
};
class B {
  template <class T> template <class U> friend int A<T>::b(U, const B &, int);
  int n = 5;
public:
  B() {}
};
template <class T> template <class U> int A<T>::b(U, const B &x) {
  if constexpr (requires { x.n; }) return x.n;
  else return 0;
}
int main() {
  B b;
  int r = A<int>::b(1, b);
  return r == 0 || r == 5 ? 0 : 1;
}
