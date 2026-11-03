//@ expect: ast
//@ ast-filter: C1
// [temp.param]/14, [temp.arg.general]/9, [dcl.ref]/1, [dcl.ref]/7: a
// written argument equal to its substituted default is dropped from a
// qualified name only where clang's structural comparison
// (`isSubstitutedDefaultArgument`) finds it equal: a qualified type
// parameter with a reference argument (`const T`, `T = int &`) and a
// collapsed reference (`T &`, `T = int &&`) are not, so the written
// argument prints; an omitted one is dropped either way.
template <class T, class A = T *, class B = const T &, class D = T &&>
struct X { struct C1 { int x; }; };
template <> struct X<long, long *, const long &, long &&>::C1 { int a; };
template <> struct X<const int, const int *, const int &>::C1 { int b; };
template <> struct X<long, long *const>::C1 { int c; };
template <class T, class R = T &> struct Y { struct C1 { int x; }; };
template <> struct Y<int &, int &>::C1 { int a; };
template <> struct Y<int &&, int &>::C1 { int b; };
template <> struct Y<int, int &>::C1 { int c; };
template <> struct Y<short &&>::C1 { int d; };
template <class T, class R = const T> struct Z { struct C1 { int x; }; };
template <> struct Z<int &, int &>::C1 { int a; };
template <> struct Z<const int, const int>::C1 { int b; };
template <> struct Z<long, const long>::C1 { int c; };
template <> struct Z<int[2], const int[2]>::C1 { int d; };
template <> struct Z<long &>::C1 { int e; };
