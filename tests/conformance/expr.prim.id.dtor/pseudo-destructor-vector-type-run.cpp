// [expr.prim.id.dtor]/2 (GNU vector extension): a vector type is destroyed
// like a scalar, so `v.~V()` on a `vector_size` type is a pseudo-destructor
// call (clang `BuildPseudoDestructorExpr` accepts a vector type), and
// libstdc++'s `is_destructible`, which probes `declval<T&>().~T()`, holds:
// a `std::vector` of SIMD vectors compiles. The shape of folly's
// `MemcpyBenchmark` (`std::vector<uint64x2_t>`). WP-726.
//@ std: c++20
//@ expect: run
//@ stdout: 3 9 10
#include <cstdio>
#include <type_traits>
#include <vector>
typedef int v4 __attribute__((vector_size(16)));
static_assert(std::is_destructible_v<v4>);
static_assert(std::is_nothrow_destructible_v<v4>);
template <class T> void destroy(T* p) { p->~T(); }
template <class T> constexpr bool probe = noexcept(static_cast<T*>(nullptr)->~T());
static_assert(probe<v4>);
int main() {
  std::vector<v4> v(2);
  v.push_back(v4{1, 9, 3, 4});
  v4 x{10, 2, 3, 4};
  destroy(&x);
  using V = v4;
  x.~V();
  std::printf("%d %d %d\n", (int)v.size(), v[2][1], x[0]);
}
