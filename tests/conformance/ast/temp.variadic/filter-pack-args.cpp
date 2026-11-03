//@ expect: ast
//@ ast-filter: C
// [temp.variadic]/7, [temp.arg.template], [temp.arg.nontype]: clang's
// -ast-dump-filter names a member of a class template specialization by
// its pack arguments' elements: template template arguments by name,
// enumeration arguments by enumerator (a value naming none without its
// cast, `7`), and the arguments of a deduced (`auto`) parameter with
// their type (`(K)9`, `3U`, `'a'`); an empty pack prints nothing.
template <class T> struct V {};
namespace q { template <class T> struct W {}; }
enum class K { a, b };
enum E { e0, e1 };
template <template <class> class... TT> struct X { struct C { int x; }; };
template <> struct X<V, q::W>::C { int i; };
template <K... ks> struct Y { struct C { int y; }; };
template <> struct Y<K::a, K::b>::C { int j; };
template <class T, K... ks> struct Y2 { struct C { int y; }; };
template <> struct Y2<int, (K)7, K::a>::C { int k; };
template <> struct Y2<long>::C { int l; };
template <auto... vs> struct Y3 { struct C { int y; }; };
template <> struct Y3<K::a, 3, (K)9, 3U, 'a', true, -1L, 5ULL, e1, (short)-3>::C { int m; };
template <auto v> struct Y4 { struct C { int y; }; };
template <> struct Y4<(K)9>::C { int n; };
template <> struct Y4<'a'>::C { int o; };
