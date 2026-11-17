// [expr.prim.id.qual]/1, [temp.alias]/2: an alias template specialization
// naming a class or an enumeration, or a dependent type, may precede ::.
//@ expect: compile-ok
//@ strict-diags
struct C { static const int v = 1; enum E { e = 2 }; };
template<typename> using A = C;
template<typename> using EA = C::E;
template<typename T> using Id = T;
template<class X> struct S { static int g() { return A<X>::v + Id<X>::v; } };
int h() { return S<C>::g() + A<int>::v + EA<int>::e; }
