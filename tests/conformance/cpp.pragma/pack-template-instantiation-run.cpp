// [cpp.pragma]: a class template's specializations take the
// `#pragma pack` value in effect where the template was defined, not
// where they are instantiated (clang copies the pattern's
// MaxFieldAlignmentAttr). LLVM's CovMapFunctionRecordV1<uint32_t> was 24
// bytes instead of 20 (WP-782).
//@ expect: run
//@ stdout: 20 1 12 14 5 21 9 | 16 8 8 | 12 1 5
#include <cstddef>
#include <cstdint>
#include <cstdio>

#pragma pack(push, 1)
template <class P> struct R {
  P n;
  uint32_t s;
  uint32_t d;
  uint64_t h;
};
template <class P> struct Q {
  P n;
  uint64_t h;
  struct In {
    char c;
    int i;
  } in;
};
#pragma pack(pop)

template <class P> struct U {
  P n;
  uint64_t h;
};

template <class P> struct O {
  template <class T> struct M {
    char c;
    T t;
    P p;
  };
};

#pragma pack(push, 1)
// Instantiated inside a pack(1) region, defined outside one: not packed.
constexpr std::size_t usz = sizeof(U<uint32_t>);
constexpr std::size_t ual = alignof(U<uint32_t>);
constexpr std::size_t uoff = offsetof(U<uint32_t>, h);
// A member template of a class template defined outside the region.
constexpr std::size_t msz = sizeof(O<int>::M<uint32_t>);
#pragma pack(pop)

int main() {
  std::printf("%zu %zu %zu %zu %zu %zu %zu | %zu %zu %zu | %zu %zu %zu\n",
              sizeof(R<uint32_t>), alignof(R<uint32_t>),
              offsetof(R<uint32_t>, h), sizeof(Q<char>),
              sizeof(Q<char>::In), sizeof(Q<int64_t>), offsetof(Q<char>, in),
              usz, ual, uoff, msz, alignof(R<char>), offsetof(R<char>, d));
}
