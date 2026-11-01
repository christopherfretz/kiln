// [temp.inst]/12 (WP-156): a default argument of a member of a class
// template specialization is instantiated only when a call uses it, so
// an ill-formed default that is never used is no error. A function-local
// class or a lambda instantiates its default arguments with the enclosing
// function. A value-initialized `T()` passes the default constructor's
// default arguments.
//@ expect: run
//@ exit: 0
template <class T> struct A {
  constexpr int f(int x = T::value) const { return x; }
  static constexpr int g(int y = T::value + 1) { return y; }
  template <class U> constexpr int h(U u = T::value * 2) const { return u; }
  constexpr A(int z = T::value) : z(z) {}
  int z;
  int bad(int q = T::nope) { return q; }
  struct In { int m(int w = sizeof(T)) { return w; } };
  template <class U> struct M { static int k(int v = sizeof(T) + sizeof(U)) { return v; } };
};
struct B { static constexpr int value = 3; };
static_assert(A<B>().f() == 3);
static_assert(A<B>::g() == 4);
static_assert(A<B>().h<int>() == 6);
static_assert(A<B>().z == 3);
struct C { constexpr C(int z = 7) : z(z) {} int z; };
static_assert(C().z == 7);

template <class T> int local() {
  struct L { int n(int a = sizeof(T)) { return a; } };
  auto lam = [](int a = sizeof(T) * 2) { return a; };
  return lam() + L().n();
}

int main() {
  A<B> b;
  A<B>::In i;
  int r = b.f() + b.bad(1) + i.m() + A<B>::M<char>::k() + A<int>(1).z;
  if (r != 3 + 1 + 1 + 2 + 1) return 1;
  if (C().z != 7) return 2;
  if (local<int>() != 12) return 3;
  return 0;
}
