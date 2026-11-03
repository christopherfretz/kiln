// WP-492, [temp.inst]/2-3: a class template specialization, a member
// class of one, or an explicit specialization is complete to code
// generation only from the end of the top-level declaration during which
// it was completed (clang emits each top-level declaration when it is
// parsed, and a namespace or `extern "C++"` block is one). A function
// first referenced earlier is declared `void ()` without attributes, as
// for a non-template class defined later (`incomplete_class_sig.cpp`).
template<class T> struct X { T t; };
template<class T> struct O { struct I { T v; }; };
template<class T> struct S;
X<int> g(X<int>);
O<int>::I gi(O<int>::I);
S<int> gs(S<int>);
void h() { auto p = &g; auto q = &gi; auto r = &gs; (void)p; (void)q; (void)r; }
template<> struct S<int> { long a; };
void k() { X<int> x{}; g(x); O<int>::I i{}; gi(i); gs(S<int>{}); }
namespace N {
template<class T> struct Y { T t; };
Y<int> gy(Y<int>);
void hy() { auto p = &gy; (void)p; }
inline void use() { Y<int> y{}; gy(y); }
void u2() { use(); }
}
X<long> gl(X<long>);
void hl() { X<long> x{}; auto p = &gl; (void)p; (void)x; }
namespace M {
struct A;
void ga(A);
void ha() { auto p = &ga; (void)p; }
struct A { int x; };
}
extern "C++" {
struct B;
void gb(B);
void hb() { auto p = &gb; (void)p; }
struct B { int x; };
}
