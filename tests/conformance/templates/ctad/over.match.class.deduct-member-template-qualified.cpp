// [temp.mem], [over.match.class.deduct]/1, [dcl.type.class.deduct]/1: a
// member class template named through the template-id of the enclosing
// specialization (`Outer<int>::In`) is a placeholder for a deduced class
// type exactly as through an alias of it (`O::In`): its guides come from
// the instantiated member, with the enclosing arguments substituted --
// including the guides of its constructor templates, whose template
// parameters follow the member's own. A constructor parameter's default
// argument gives the guide's parameter a default argument that is never
// substituted or evaluated by deduction.
//@ strict-diags
struct S { static constexpr int value = 3; };
template<class T> struct Outer {
  template<class U> struct In { In(T, U); };
  template<class U> struct H { template<class V> H(V, U); };
  template<class... Us> struct P { template<class V> P(V, T, Us...); };
  template<class U> struct Q { Q(U, int = T::value) {} };
  template<class U> struct R { template<class V> R(V, U, int = V::value + T::value) {} };
};
Outer<int>::In x(1, 2.0);
static_assert(__is_same(decltype(x), Outer<int>::In<double>));
auto y = Outer<int>::In(1, 'c');
static_assert(__is_same(decltype(y), Outer<int>::In<char>));
auto *z = new Outer<int>::In(1, 2L);
static_assert(__is_same(decltype(z), Outer<int>::In<long> *));
Outer<int>::In w{1, 2.0f};
static_assert(__is_same(decltype(w), Outer<int>::In<float>));
Outer<int>::H h(1.0, 2);
static_assert(__is_same(decltype(h), Outer<int>::H<int>));
using O = Outer<int>;
O::H h2(1.0, 'c');
static_assert(__is_same(decltype(h2), Outer<int>::H<char>));
O::P p(1.0, 2, 'c', 3L);
static_assert(__is_same(decltype(p), Outer<int>::P<char, long>));
using OS = Outer<S>;
OS::Q q(1.0);
static_assert(__is_same(decltype(q), Outer<S>::Q<double>));
OS::R r(S{}, 2);
static_assert(__is_same(decltype(r), Outer<S>::R<int>));
Outer<S>::R r2(S{}, 2L, 3);
static_assert(__is_same(decltype(r2), Outer<S>::R<long>));
template<class T> struct A { template<class V> A(T, V, int = V::value) {} };
A a(1, S{});
static_assert(__is_same(decltype(a), A<int>));
