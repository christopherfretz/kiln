// [over.match.class.deduct]/1, [dcl.type.class.deduct]/1: class template
// argument deduction for a member class template of a class template
// specialization uses the guides of every constructor of the
// instantiated member template, constructor templates included, and the
// template may be named by a qualified name through a template-id
// (`Outer<int>::In`) as through an alias of the specialization.
//@ std: c++17
//@ expect: compile-ok
//@ strict-diags
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;
template <class T> struct Outer {
  template <class U> struct In {
    template <class V> In(U, V) {}
    In(U) {}
  };
};
Outer<int>::In a(1, 2.0);
Outer<int>::In b(1L);
using O = Outer<int>;
O::In c('x', 1);
static_assert(same<decltype(a), Outer<int>::In<int>>, "");
static_assert(same<decltype(b), Outer<int>::In<long>>, "");
static_assert(same<decltype(c), Outer<int>::In<char>>, "");
