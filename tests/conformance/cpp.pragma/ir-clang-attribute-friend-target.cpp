// [cpp.pragma] [class.friend]/6: a function first declared or defined as
// a friend inside a class takes the attributes of an enclosing
// '#pragma clang attribute push(..., apply_to = function)' region like any
// other function (clang ProcessDeclAttributes -> AddPragmaAttributes):
// a pushed target("avx2") reaches a hidden friend, a friend operator, the
// hidden friend of a class template (and so its instantiation) and a
// lambda's call operator, so each may inline the always_inline AVX2
// intrinsic and carries +avx2 in its attribute group. Found building
// open-source SIMD libraries. spec/sema.md "#pragma clang attribute".
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -ftrivial-auto-var-init=uninitialized
// The AVX2 operations are declared here as <immintrin.h> declares them
// (always_inline + target("avx2")) so the test reaches no libc header
// under its pinned --target.
typedef long long __m256i __attribute__((__vector_size__(32), __aligned__(32)));
typedef char __v32qi __attribute__((__vector_size__(32)));
typedef unsigned char __v32qu __attribute__((__vector_size__(32)));
static inline __attribute__((__always_inline__, __target__("avx2"), __min_vector_width__(256)))
__m256i _mm256_cmpeq_epi8(__m256i a, __m256i b) { return (__m256i)((__v32qi)a == (__v32qi)b); }
static inline __attribute__((__always_inline__, __target__("avx2"), __min_vector_width__(256)))
__m256i _mm256_add_epi8(__m256i a, __m256i b) { return (__m256i)((__v32qu)a + (__v32qu)b); }

#pragma clang attribute push(__attribute__((target("avx2"))), apply_to = function)
struct M {
  __m256i raw;
  friend inline M eq(M a, M b) { return M{_mm256_cmpeq_epi8(a.raw, b.raw)}; }
  friend inline M operator==(M a, M b) { return M{_mm256_cmpeq_epi8(a.raw, b.raw)}; }
};
template <class T> struct V {
  T raw;
  friend V add(V a, V b) { return V{_mm256_add_epi8(a.raw, b.raw)}; }
};
inline M through_lambda(M a, M b) {
  return [](M x, M y) { return M{_mm256_cmpeq_epi8(x.raw, y.raw)}; }(a, b);
}
#pragma clang attribute pop

M f(M a, M b) { return eq(a, b); }
M g(M a, M b) { return a == b; }
V<__m256i> h(V<__m256i> a, V<__m256i> b) { return add(a, b); }
M k(M a, M b) { return through_lambda(a, b); }
