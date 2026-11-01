// [class.mfct.non.static]/2, [temp.dep.type]: in a member of a class
// template with a dependent base, a qualified name `B::m` naming a
// non-static member of a class that may be a base at instantiation is an
// implicit member access `(*this).B::m`, resolved when the class is
// instantiated (libc++ `basic_ios` calling `ios_base::flags()` through
// `basic_ios<_CharT, _Traits>`). `&B::m` is a pointer to member, never an
// implicit member access. Verified against clang 19.
//@ expect: run
struct B {
  int x = 7;
  int h() { return x; }
  int f() { return 1; }
  int f(int v) { return v; }
};
template <class T> struct M : T {
  int g() { return B::h() + B::f() + B::f(2) + B::x; }
  static constexpr int (B::*ph)() = &B::h;
  static constexpr int B::*px = &B::x;
  int k() { return (this->*ph)() + this->*px; }
};
int main() {
  M<B> m;
  if (m.g() != 17) return 1;
  if (m.k() != 14) return 2;
  int (B::*pf)(int) = &B::f;
  if ((m.*pf)(5) != 5) return 3;
  return 0;
}
