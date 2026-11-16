// [temp.arg.template]/1, [temp.res.general]/3: without 'template', a
// member of an unknown specialization does not name a template, so it is
// not a template template argument. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 10:42 error "template argument for template template parameter must be a class template or type alias template"
template <int> struct A { template <int> struct B { enum { v = 1 }; }; };
template <template <int> class F> struct C { enum { v = F<1>::v }; };
template <int n> struct D { enum { v = C<A<n>::B>::v }; };
// Valid: with `template`, a non-dependent qualifier, and a qualifier
// whose specialization is not instantiated yet.
template <int n> struct E { enum { v = C<A<n>::template B>::v }; };
E<1> e;
int k = C<A<2>::B>::v;
template <template <class...> class TT> struct quote {
  template <class... Ts> using apply = TT<Ts...>;
};
template <class...> struct L {};
using t = quote<quote<L>::apply>::apply<int>;
