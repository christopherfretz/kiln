// A dependent GNU `__alignof__` in a default template argument that a
// function template's signature reaches through a dependent member type
// ([expr.alignof], [temp.over.link]/5; spec/abi.md "Mangling",
// WP-1226): the shape of libstdc++'s `aligned_storage<sizeof(T)>::type`,
// whose `_Align` defaults to `__alignof__(typename msa<Len>::type)`.
// clang 19 and 22 mangle the default as the vendor extended expression
// `Xu11__alignof__ <type> EE`, next to `st <type>` for `sizeof(T)` and
// `at <type>` for `alignof(T)`. kiln rejected it with "cannot yet mangle
// expression type UnaryExprOrTypeTraitExpr".
template <unsigned long L> struct msa {
  struct type {
    alignas(16) unsigned char data[L];
  };
};
template <unsigned long L, unsigned long Al = __alignof__(typename msa<L>::type)>
struct storage {
  struct type {
    alignas(Al) unsigned char data[L];
  };
};
template <class T> int f(typename storage<sizeof(T)>::type *p) {
  return sizeof(*p) == sizeof(typename storage<sizeof(T)>::type) ? 0 : 1;
}
template <class T, unsigned long N> struct B {};
template <class T> int g(B<T, alignof(T)> *) { return 2; }
template <class T> int h(B<T, __alignof__(T) + 1> *) { return 3; }
int use() {
  storage<sizeof(long)>::type s;
  return f<long>(&s) + g<int>(nullptr) + h<int>(nullptr);
}
