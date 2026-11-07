// P3865R3 ([over.match.class.deduct], a DR): class template argument
// deduction through a template template parameter, naming a class
// template or an alias template.
//@ expect: compile-ok
//@ std: c++26
//@ strict-diags
template <class T> struct W { W(T) {} T v{}; };
template <template <class> class TT> auto f() { TT x(1); return x; }
static_assert(__is_same(decltype(f<W>()), W<int>));
template <class T> struct A2 { A2(T, T) {} };
template <class T> using AA = A2<T>;
template <template <class> class TT> auto g() { TT x(1, 2); return x; }
static_assert(__is_same(decltype(g<AA>()), A2<int>));
