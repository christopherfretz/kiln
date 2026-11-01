template <class T, bool B = false, class U = T> struct X { struct C { int x; }; };
template <class T> struct X<T *> { struct C { int p; }; };
template <> struct X<int, false, long>::C { int j; };
template <template <class> class TT, class T> struct D { struct C { TT<T> m; }; };
template <class T> struct E {};
namespace q { template <class T> struct F {}; }
template <> struct D<E, int> { struct C { int e; }; };
template <> struct D<q::F, int>::C { int f; };
enum class K { a, b };
template <K k> struct G { struct C {}; };
template <> struct G<K::b>::C { int g; };
template <> struct G<K(7)>::C { int h; };
