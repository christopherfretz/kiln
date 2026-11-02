// [temp.inst]/2,3 (a class template specialization is implicitly
// instantiated where a complete type is required), [temp.alias]/2 (an
// alias template specialization names the class template specialization it
// stands for), Itanium C++ ABI §2.4: the layout dump names a specialization
// with its template arguments as clang's `-fdump-record-layouts` does
// (canonical arguments, tag keywords, `> >`), and dumps each instantiation
// where it was first required, before the class whose member required it.
//@ expect: layout
template <class T, int N> struct S { T a[N]; };
template <class T> struct P { T t; };
template <class... Ts> struct V { int x; };
struct Q { int q; };
namespace ns { template <class T> struct W { T w; }; }
template <class T> using A = P<T>;
template <bool B, char C, unsigned long L> struct N { int n; };
enum E { e0 };
template <E e> struct EV { int v; };
template <class T> struct Outer { template <class U> struct In { T t; U u; }; };
S<int, 4> s1;
P<Q> p1;
P<P<char>> p2;
V<> v0;
V<int, Q, double> v1;
ns::W<int *> w1;
A<long> a1;
N<true, 'a', 3> n1;
EV<e0> ev;
P<int[3]> p3;
P<int (*)(int)> p4;
P<E> p5;
Outer<int>::In<char> oi;
struct H { S<char, 2> s; P<Q> p; };
H h;
// Default arguments, typedef'd arguments, explicit and partial
// specializations, unions and classes.
template <class T, class U = int> struct D { T t; U u; };
D<char> d1;
typedef short Short;
D<Short, Short> d2;
template <class T> struct X { T x; };
template <> struct X<char> { int special; };
template <class T> struct X<T *> { T *ptr; long l; };
X<char> x1;
X<double *> x2;
template <class T> union U1 { T a; char c; };
U1<int> u1;
template <class T> class C1 { public: T c; };
C1<Q> c1;
// Substituted parameters in fields that are not expanded.
template <class T> struct F { T *p; T &r; T a[2]; P<T> *pp; };
struct HasF { F<Q> *f; };
template <class T> struct G { T *p; T a[2]; enum E e; };
G<Q> g1;
G<E> g2;
G<Short> g3;
// Nested classes of a specialization, and a written specialization type.
template <class T> struct Out2 { struct Inner { T i; }; Inner in; };
Out2<long> o2;
namespace ns { struct R { int r; }; }
struct Plain { P<P<int>> *pp; ns::W<ns::R> w; };
Plain plain;
// Template template, nullptr and enum class arguments.
template <template <class> class TT> struct TTA { TT<int> m; };
TTA<P> tta;
enum class EC { a, b };
template <EC v, decltype(nullptr) n, int *ip> struct NT { int x; };
NT<EC::b, nullptr, nullptr> nt;
