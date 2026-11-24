// [temp.expl.spec]/2 (CWG 727): a class-scope explicit specialization of a
// member function template in a class template is a specialization in
// every instantiation of the class, and its definition is the one written
// in the class, instantiated with the class's arguments: deduced (`d`),
// with explicit type arguments (`s<char>`), of an operator template,
// with a non-type argument no parameter deduces (`c<0>`), in constant
// evaluation and in an explicit instantiation ([temp.explicit]/10).
//@ expect: run
//@ stdout: 1 11 10 21 20 8
extern "C" int printf(const char *, ...);
template <class T> struct A {
  template <class U> constexpr int d(U) const { return 0; }
  template <> constexpr int d(int) const { return 1; }
  template <class U> static int s(U) { return 10; }
  template <> int s<char>(char) { return 11; }
  template <class U> int operator()(U) { return 20; }
  template <> int operator()(T) { return 21; }
  template <int N> constexpr int c() const { return N; }
  template <> constexpr int c<0>() const { return sizeof(T); }
};
static_assert(A<int>{}.d(1) == 1 && A<int>{}.d(1.0) == 0);
static_assert(A<long>{}.c<0>() == sizeof(long) && A<long>{}.c<3>() == 3);
template struct A<short>;
int main() {
  A<double> a;
  printf("%d %d %d %d %d %d\n", a.d(1), a.s('x'), a.s(1), a(1.0), a(1), a.c<0>());
}
