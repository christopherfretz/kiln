// [temp.deduct.general]/8, [dcl.init.list]/7: a narrowing conversion inside
// `decltype` in an alias template or a trailing return type is a
// substitution failure, and stays one when the same specialization is
// named again (the memoized alias and function specializations must not be
// reused as valid). libc++ variant's `__overload` chain picks the only
// non-narrowing alternative.
//@ expect: compile-ok
//@ strict-diags
template <class T> T&& declval();
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };

template <class D> auto t(D (&&)[1]) -> D*;
template <class D, class S> using Apply = decltype(t<D>({declval<S>()}));
template <class T> struct ov { template <class U> auto operator()(T, U&&) const -> Apply<T, U>; };
struct all : ov<unsigned>, ov<long> { using ov<unsigned>::operator(); using ov<long>::operator(); };
static_assert(same<decltype(all()(0, declval<int>())), long*>::value);
static_assert(same<decltype(all()(0, declval<int>())), long*>::value);

struct nc {
  template <class D> static auto t(D (&&)[1]) -> D*;
  template <class D, class S> using Apply = decltype(t<D>({declval<S>()}));
};
template <class D, class S> using Check = typename nc::template Apply<D, S>;
template <class T, int I> struct ov2 { template <class U> auto operator()(T, U&&) const -> Check<T, U>; };
template <class... B> struct all2 : B... { using B::operator()...; };
static_assert(same<decltype(all2<ov2<float, 0>, ov2<long, 1>>()(0, declval<int>())), long*>::value);
static_assert(same<decltype(all2<ov2<short, 0>, ov2<long, 1>, ov2<float, 2>>()(0, declval<int>())), long*>::value);
