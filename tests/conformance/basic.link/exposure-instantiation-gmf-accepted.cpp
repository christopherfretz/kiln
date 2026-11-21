// [basic.link]/16.5: kiln follows only the instantiations of the module
// purview's own templates. A template of the global module fragment (the
// standard library) instantiated with a type whose TU-local operator its
// body finds by argument-dependent lookup is not checked, so an exported
// inline function calling it stays valid with no diagnostic, as in clang
// 22. GCC 16.1 rejects `std::less<void>::operator()`'s instantiation.
//@ expect: compile-ok
//@ std: c++20
//@ skip(gcc): GCC 16.1 rejects the standard library's instantiation
//@ strict-diags
module;
#include <algorithm>
#include <vector>
export module M;
namespace N {
struct A { int v; };
static bool operator<(A a, A b) { return a.v < b.v; }
}
export inline void sorted(std::vector<N::A> &v) { std::sort(v.begin(), v.end()); }
export inline bool found(const std::vector<int> &v) {
  return std::find(v.begin(), v.end(), 3) != v.end();
}
