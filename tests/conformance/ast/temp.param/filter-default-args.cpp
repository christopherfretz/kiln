//@ expect: ast
//@ ast-filter: C
// [temp.param]/14, [temp.arg.general]/9, [temp.arg.template]: clang's
// -ast-dump-filter names a member of a class template specialization
// without the trailing arguments equal to their defaults
// (`isSubstitutedDefaultArgument`): a type default naming earlier
// parameters is compared after substitution (`U = T *`, `const T &`,
// `T **`, `T &&`, references collapsed, qualifiers merged), and a
// template template default by the template it names, however the
// argument spells it (`q::W`, `W`).
template <class T> struct V {};
namespace q { template <class T> struct W {}; }
using namespace q;
template <class T, class A = const T &, class B = T **, class C = T &&,
          class D = const T *, class E = volatile T *const, class F = T>
struct X { struct C1 { int x; }; };
template <> struct X<int>::C1 { int a; };
template <> struct X<int *>::C1 { int b; };
template <> struct X<const int>::C1 { int c; };
template <> struct X<long, const long &, long **, long &&, const long *,
                     volatile long *>::C1 { int d; };
template <class T, class A = T &, class B = T &&, class C = const T &>
struct R { struct C1 { int r; }; };
template <> struct R<int &>::C1 { int e; };
template <> struct R<int &&>::C1 { int e; };
template <> struct R<int &&, int &, int &>::C1 { int e; };
template <class T, template <class> class TT = W, template <class> class UU = V>
struct Y { struct C1 { int y; }; };
template <> struct Y<int>::C1 { int f; };
template <> struct Y<short, q::W, V>::C1 { int g; };
template <> struct Y<long, V>::C1 { int h; };
template <> struct Y<char, W, W>::C1 { int i; };
