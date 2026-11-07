//@ expect: run
//@ stdout: 32 64 7 5 3
// WP-782: [temp.local]/7: in the out-of-line definition of a member
// template, the members of the class (and of its bases) hide the template
// parameters of enclosing class templates but not the member's own
// template parameters (LLVM's AArch64InstPrinter
// `template <unsigned size> void printGPRSeqPairsClassOperand(...)`
// with a base `size()` in scope).
#include <cstdio>

struct Base {
  int size() const { return 1; }
  int size(int) const { return 2; }
  static const int n = 9;
};
struct P : Base {
  template <unsigned size> int f() const;
  template <int n> static int g();
};
template <unsigned size> int P::f() const {
  static_assert(size == 64 || size == 32, "");
  return size;
}
template <int n> int P::g() { return n; }

template <class T> struct A : Base {
  static const int m = 3;
  template <int size> int h() const;
  template <int m> int k() const;
};
template <class T> template <int size> int A<T>::h() const { return size; }
// The member's own `m`, not the class member `m`.
template <class T> template <int m> int A<T>::k() const { return m; }

int main() {
  P p;
  A<int> a;
  std::printf("%d %d %d %d %d\n", p.f<32>(), p.f<64>(), P::g<7>(), a.h<5>(),
              a.k<3>());
}
