// [temp.spec.partial.member], [temp.constr.decl]: constrained partial
// specializations of a member class template of a class template; the
// partial specialization's template parameters are in scope in its member
// bodies. libstdc++ bits/iterator_concepts.h:395 (__iter_traits_impl
// __ptr<_Iter> requires typename _Iter::pointer), ranges:1888 / chrono:2449
// (params `_Const`, `_Rep`, `_Period` not found), bits/stl_list.h:189;
// 26 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): a member partial specialization's template parameters are not in scope in its member function bodies (`Const`)
template<class It> struct traits {
private:
  template<class I> struct ptr { using type = void; };
  template<class I> requires requires { typename I::pointer; }
  struct ptr<I> { using type = typename I::pointer; };
public:
  using pointer = typename ptr<It>::type;
};
struct A { using pointer = int *; };
struct B {};
static_assert(sizeof(traits<A>::pointer) == sizeof(int *));
traits<B>::pointer *vp = nullptr;
template<class V> struct view {
  template<bool Const> using Base = V;
  template<bool Const> struct cat {};
  template<bool Const> requires (sizeof(Base<Const>) > 0)
  struct cat<Const> {
    static auto get() { using B = view::Base<Const>; return B(); }
  };
};
static_assert(sizeof(view<int>::cat<true>::get()) == sizeof(int));
