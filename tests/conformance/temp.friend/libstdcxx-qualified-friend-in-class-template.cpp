// [temp.friend]/1, [dcl.meaning]/1: a qualified friend function template
// declaration inside a class template refers to the namespace-scope template
// it names (the same declaration in a non-template class already works).
// libstdc++ bits/regex.h:816 (friend __detail::__regex_algo_impl),
// format:3713 (friend std::make_format_args); 27 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): friend declaration of a qualified function template in a class template does not match
namespace N { template<class T> void f(T); template<class T, class U> struct box {}; template<class I, class A> void g(box<I, A> &); }
template<class X> class R { template<class T> friend void N::f(T); };
class S { template<class T> friend void N::f(T); };
template<class X> class R2 { template<class B, class A> friend void N::g(N::box<B, A> &); };
class S2 { template<class B, class A> friend void N::g(N::box<B, A> &); };
template class R<int>; template class R2<int>;
