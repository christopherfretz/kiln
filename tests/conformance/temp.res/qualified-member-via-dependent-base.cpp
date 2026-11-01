// [temp.dep.type]/6, [class.mfct.non.static]/2: inside a member of a class
// template with a dependent base, a qualified name `B::m` of a non-static
// member of a class not yet known to be a base is an implicit
// `(*this).B::m` resolved at instantiation (clang `IMA_Dependent`). The
// libc++ `basic_ostream::operator<<(int)` pattern (`ios_base::flags()`
// through the virtual base `basic_ios<C>`).
//@ expect: run
//@ stdout: 5523 7
extern "C" int printf(const char*, ...);

struct B {
  int v = 5;
  int get() const { return v; }
  int ov(int) const { return 1; }
  int ov(long) const { return 2; }
  template <class X> int t(X) const { return 3; }
  static const int basefield = 7;
};
template <class T> struct Mid : B {};
template <class T> struct D : virtual Mid<T> {
  int f() const;
  int g() const { return B::get() & B::basefield; }
};
template <class T> int D<T>::f() const {
  return B::get() * 1000 + B::v * 100 + B::ov(1L) * 10 + B::t(1);
}

struct U { int w() const { return 7; } };
template <class T> struct E : T { int h() const { return U::w(); } };
struct F : U {};

int main() {
  D<int> d;
  printf("%d %d\n", d.f() + (d.g() - 5), E<F>().h());
}
