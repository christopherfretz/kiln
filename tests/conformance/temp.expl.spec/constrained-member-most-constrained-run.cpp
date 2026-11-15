// [temp.expl.spec]/1, [temp.constr.order]/5, [over.match.best.general]/2.6:
// `template<> void S<X>::f()` where `S<T>` declares both `void f()` and
// `void f() requires C<T>` explicitly specializes the member whose
// constraints `X` satisfies and, of two, the more constrained one; with
// `S<int>` only the unconstrained one is a candidate. kiln took the first
// member with the type, so the specialization replaced the wrong member
// and the call ran the other one (found running GCC's C++ testsuite).
// Also with two constrained members (`requires C<T>` vs
// `requires D<T>`, D subsuming C) and for a static member function.
//@ expect: run
//@ std: c++20
//@ stdout: 0 1 2 0
//@ stdout: 3 4
//@ stdout: 5 0
extern "C" int printf(const char *, ...);
template <class T>
concept C = __is_class(T);
template <class T>
concept D = C<T> && __is_empty(T);

struct X {};
struct Y {
  int n;
};

template <class T>
struct S {
  int f() { return 0; }
  int f() requires C<T> { return 0; }
  int g() requires C<T> { return 0; }
  int g() requires D<T> { return 0; }
  static int h() { return 0; }
  static int h() requires D<T> { return 0; }
};

template <>
int S<int>::f() { return 1; }
template <>
int S<X>::f() { return 2; }
template <>
int S<X>::g() { return 3; }
template <>
int S<Y>::g() { return 4; }
template <>
int S<X>::h() { return 5; }

int main() {
  S<double> sd;
  S<int> si;
  S<X> sx;
  S<Y> sy;
  printf("%d %d %d %d\n", sd.f(), si.f(), sx.f(), sy.f());
  printf("%d %d\n", sx.g(), sy.g());
  printf("%d %d\n", S<X>::h(), S<Y>::h());
}
