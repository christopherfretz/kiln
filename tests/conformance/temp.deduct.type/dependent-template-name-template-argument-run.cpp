// [temp.deduct.type]/5.1: `X<T::template B, T>` is chosen for
// `X<A::B, A>` and not for `X<A::B, int>`, observed at run time (found
// running GCC's C++ testsuite).
//@ expect: run
//@ stdout: 1 0
extern "C" int printf(const char *, ...);
struct A {
  template <class>
  struct B {};
};
template <template <class> class TT, class T>
struct X {
  int f() { return 0; }
};
template <class T>
struct X<T::template B, T> {
  typename T::template B<int> y;
  int f() { return 1; }
};
int main() {
  X<A::B, A> a;
  X<A::B, int> b;
  printf("%d %d\n", a.f(), b.f());
}
