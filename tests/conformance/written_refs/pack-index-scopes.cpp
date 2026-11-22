// The written-reference table (spec/ast.md §18.4), a pack-index-specifier
// ([dcl.type.pack.index]) as the first component of a
// nested-name-specifier: the pack's name has role `nns` and qualifies
// the next name, as a class name would; as a type-specifier and as a
// base it is a `type` and `base` name.
//@ written-refs: pack-index-scopes.refs
//@ std: c++26
struct A {
  using type = int;
  static constexpr int v = 1;
};
template <class... Ts> struct L {
  using first = typename Ts...[0]::type;
  using plain = Ts...[0];
  static constexpr int sum = Ts...[0]::v;
};
template <class... Ts> struct D : Ts...[0] {};
int x = L<A>::sum + D<A>::v;
