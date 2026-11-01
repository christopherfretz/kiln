// A dependent ext_vector_type / vector_size alias template is a
// DependentVector type: it can be a partial specialization's argument
// (variable and class templates), a deduced function parameter, and the
// `sizeof` of a dependent mask vector stays dependent in the pattern
// (libc++ __algorithm/simd_utils.h:101 and :148; WP-151).
typedef unsigned long size_t;
template <class T, size_t N>
using vec __attribute__((__ext_vector_type__(N))) = T;
template <class T, size_t B>
using gvec __attribute__((vector_size(B))) = T;

template <class V>
inline constexpr size_t vsize = 0;
template <class T, size_t N>
inline constexpr size_t vsize<vec<T, N>> = N;
static_assert(vsize<vec<int, 4>> == 4);
static_assert(vsize<vec<bool, 64>> == 64);
static_assert(vsize<int> == 0);

template <class V> struct VS { static constexpr size_t v = 0; };
template <class T, size_t N> struct VS<vec<T, N>> { static constexpr size_t v = N; };
static_assert(VS<vec<float, 8>>::v == 8);
static_assert(VS<gvec<float, 16>>::v == 0); // a vector_size vector is not an ext vector

template <class T, size_t N> T under(vec<T, N>) { return T{}; }
static_assert(__is_same(decltype(under(vec<char, 16>{})), char));

static_assert(__is_same(vec<int, 4>, vec<int, 2 + 2>));
static_assert(__is_same(gvec<int, 16>, vec<int, 4>) == false);
static_assert(sizeof(gvec<short, 8>) == 8);

template <class T, size_t N>
size_t mask_bytes(vec<T, N>) {
  using mask = vec<bool, N>;
  if constexpr (sizeof(mask) == 1) {
    return 1;
  } else if constexpr (sizeof(mask) == 2) {
    return 2;
  } else {
    static_assert(sizeof(mask) == 0, "unexpected mask size");
    return 0;
  }
}
size_t m8 = mask_bytes(vec<int, 8>{});
size_t m16 = mask_bytes(vec<char, 16>{});

// vector_size also applies to variables, fields and parameters.
int w __attribute__((vector_size(16)));
static_assert(sizeof(w) == 16);
struct S { float f __attribute__((vector_size(8))); };
static_assert(sizeof(S) == 8 && alignof(S) == 8);
void g(int p __attribute__((vector_size(16)))) { static_assert(sizeof(p) == 16); }
template <class T> struct X { T v __attribute__((vector_size(4 * sizeof(T)))); };
static_assert(sizeof(X<short>) == 8 && alignof(X<short>) == 8);
