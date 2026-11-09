// [temp.variadic]/7, [expr.prim.fold]/3: a fold-expression in a generic
// lambda names both a function parameter pack of the enclosing function
// template and the lambda's own template parameter pack. Instantiating
// the enclosing function substitutes the parameter pack (kept as a
// FunctionParmPackExpr naming its elements) and leaves the lambda's pack;
// instantiating the call operator then expands both. With both packs
// empty the fold is its init value. libc++ 22 <mdspan> (`layout_stride`,
// `layout_right` `operator()` of a rank-0 mapping) has this shape; the
// expansion used to stay unexpanded ("constant evaluation of CXXFoldExpr").
//@ expect: compile-ok
template <unsigned long... I> struct seq {};

template <class... Ix>
constexpr int deduced(Ix... ix) {
  return [&]<unsigned long... P>(seq<P...>) {
    return ((static_cast<int>(ix) * P) + ... + 7);
  }(seq<sizeof(Ix)...>{});
}
static_assert(deduced() == 7);
static_assert(deduced(1, 2) == 7 + 4 + 8);

template <class... Ix>
constexpr int written(Ix... ix) {
  return [&]<unsigned long... P>(seq<P...>) -> int {
    return (7 + ... + (P * ix));
  }(seq<>{});
}
static_assert(written() == 7);

// A comma fold over the same two empty packs is `void()`.
template <class... Ix>
constexpr int comma(Ix... ix) {
  int n = 0;
  [&]<unsigned long... P>(seq<P...>) { ((n += static_cast<int>(ix) * P), ...); }(seq<>{});
  return n + 3;
}
static_assert(comma() == 3);

// Near miss: the pack of the enclosing template is a template parameter
// pack, not a function parameter pack (already expanded correctly).
template <int... Ix>
constexpr int nttp() {
  return []<unsigned long... P>(seq<P...>) { return ((Ix * P) + ... + 7); }(seq<>{});
}
static_assert(nttp() == 7);

int main() { return deduced() - 7; }
