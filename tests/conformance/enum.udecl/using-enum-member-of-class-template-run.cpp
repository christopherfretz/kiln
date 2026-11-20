// [enum.udecl]/1, /3, [temp.inst]/3: a using-enum-declaration in a class
// template (naming a namespace-scope enumeration, one in a namespace, a
// member enumeration of a non-template class) is instantiated with each
// specialization: its enumerators are members of the specialization,
// usable unqualified in its members and as `Hog<N>::OINK`. Reduced from
// g++.dg cpp2a/using-enum-10a.C.
//@ expect: run
//@ std: c++20
//@ stdout: 14 10 102 4
#include <cstdio>
enum class Pig { OINK = 3, SNORT };
namespace n { enum class Cow { MOO = 10 }; }
template <int I> struct Hog {
  using enum Pig;
  using enum n::Cow;
  int v = int(OINK) + int(MOO) + I;
  static constexpr Pig p = SNORT;
  Hog() = default;
  Hog(Pig x) : v(int(x)) {}
};
struct Outer { enum class Inner { A = 100, B }; };
template <class T> struct Farm {
  using enum Outer::Inner;
  static int get() { return int(B) + sizeof(T); }
};
template <int N> int pen() {
  Hog<N> h(Hog<N>::SNORT);
  return h.v;
}
int main() {
  Hog<1> h;
  static_assert(Hog<2>::p == Pig::SNORT);
  static_assert(Hog<2>::OINK == Pig::OINK);
  std::printf("%d %d %d %d\n", h.v, int(Hog<5>::MOO), Farm<char>::get(), pen<0>());
}
