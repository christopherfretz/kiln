// [temp.mem.class], [temp.spec.partial.member], [temp.over.link]/7: an
// out-of-line member definition of a constrained partial specialization
// repeats its template-head and requires-clause. libstdc++ ranges:6380
// (chunk_view<_Vp>::_Iterator for the forward_range specialization);
// <ranges>, <generator>, 2 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): requires clause differs in template redeclaration
template<typename T> concept C = sizeof(T) > 0;
template<typename T> concept F = sizeof(T) >= 1;
template<C V> class chunk_view {
  template<bool Const> class Iterator;
public:
  int get();
};
template<C V> requires F<V> class chunk_view<V> {
  template<bool Const> class Iterator;
public:
  int get();
};
template<C V> requires F<V>
template<bool Const>
class chunk_view<V>::Iterator { public: int v = Const ? 1 : 2; };
template<C V> requires F<V>
int chunk_view<V>::get() { return Iterator<true>{}.v; }
int main() { return chunk_view<int>{}.get() - 1; }
