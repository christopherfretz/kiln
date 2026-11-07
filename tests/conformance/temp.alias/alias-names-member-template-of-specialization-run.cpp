// [temp.alias]/2, [temp.res.general]/4: an alias template whose pattern
// names a member class template of a class template specialization
// (`TO<B, short>::FSS<T>`, with `TO<B, short>` instantiated but its
// member template's definition not yet) names a member template in
// `G<short>::W<1>::type`, so `sizeof(...)` and `alignof(...)` of it take
// a type-id, also through a member alias of a derived class (LLVM's
// `Class1::FixedSizeStorage<short>::with_counts<1>::type`,
// TrailingObjects.h).
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 1 7 3 4 2
#include <cstddef>
#include <cstdio>
template <class B, class... T> struct TO {
  static constexpr std::size_t total(std::size_t n) { return sizeof(B) + n * 2; }
  template <typename... Tys> struct FSS {
    template <std::size_t... Counts> struct W {
      enum { Size = total(Counts...) };
      struct type { alignas(B) char buffer[Size]; };
    };
    struct Plain { struct type { char b[3]; }; };
  };
};
struct C1 : TO<C1, short> {
  unsigned x;
  template <typename... Ty> using FSS = TO::FSS<Ty...>;
};
struct Byte { char c; };
template <typename T> using G = TO<Byte, short>::FSS<T>;
static_assert(alignof(C1::FSS<short>::W<1>::type) == alignof(C1), "");
static_assert(sizeof(C1::FSS<short>::W<1>::type) == 8, "");
int main() {
  G<short>::W<0>::type a;
  std::printf("%zu %zu %zu %zu %zu\n", sizeof(a), sizeof(G<short>::W<3>::type),
              sizeof(G<int>::Plain::type), alignof(C1::FSS<short>::W<2>::type),
              sizeof(TO<Byte>::FSS<>::W<0>::type) * 2);
}
