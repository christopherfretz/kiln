// [over.match.class.deduct]/3: an alias template whose defining-type-id
// has a non-trailing pack expansion (`tuple<T..., int>`). Its guides'
// function parameter pack is then not trailing, a non-deduced context
// deduced empty ([temp.deduct.call]/1), so no guide is viable. A guide
// whose return type cannot be deduced from the alias's (`pair<A, B>`'s
// `tuple<A, B>`) is formed with nothing deduced (clang 19), and every
// note names the guide as synthesized.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:6 error "no viable constructor or deduction guide for deduction of template arguments of 'Tail'"
//@ diag: 37:22 note "candidate function [with T = <>] not viable: requires 1 argument, but 2 were provided"
//@ diag: 37:22 note "implicit deduction guide declared as 'template <class ...T> requires __is_deducible(Tail, tuple<T..., int>) Tail(const T &..., const int &) -> tuple<T..., int>'"
//@ diag: 37:22 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 37:22 note "implicit deduction guide declared as 'template <class ...T, class A, class B> requires __is_deducible(Tail, tuple<T..., int>) Tail(const pair<A, B> &) -> tuple<T..., int>'"
//@ diag: 37:22 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 37:22 note "implicit deduction guide declared as 'template <class ...T> requires __is_deducible(Tail, tuple<T..., int>) Tail(tuple<T..., int>) -> tuple<T..., int>'"
//@ diag: 37:22 note "candidate function [with T = <>] not viable: requires 1 argument, but 2 were provided"
//@ diag: 37:22 note "implicit deduction guide declared as 'template <class ...T> requires __is_deducible(Tail, tuple<T..., int>) Tail(T ..., int) -> tuple<T..., int>'"
//@ diag: 37:22 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 37:22 note "implicit deduction guide declared as 'template <class A, class B> requires __is_deducible(Tail, tuple<A, B>) Tail(pair<A, B>) -> tuple<A, B>'"
//@ diag: 37:22 note "candidate function template not viable: requires at least 3 arguments, but 2 were provided"
//@ diag: 37:22 note "implicit deduction guide declared as 'template <class ...T, class Alloc> requires __is_deducible(Tail, tuple<T..., int>) Tail(alloc_tag, const Alloc &, const T &..., const int &) -> tuple<T..., int>'"
//@ diag: 37:22 note "candidate function template not viable: requires at least 3 arguments, but 2 were provided"
//@ diag: 37:22 note "implicit deduction guide declared as 'template <class ...T, class Alloc> requires __is_deducible(Tail, tuple<T..., int>) Tail(alloc_tag, Alloc, T ..., int) -> tuple<T..., int>'"
template<class A, class B> struct pair { A a; B b; };
struct alloc_tag {};

template<class... T> struct tuple {
  tuple(const T&...) {}
  template<class Alloc> tuple(alloc_tag, const Alloc&, const T&...) {}
  template<class A, class B> tuple(const pair<A, B>&) {}
};
template<class... T> tuple(T...) -> tuple<T...>;
template<class A, class B> tuple(pair<A, B>) -> tuple<A, B>;
template<class Alloc, class... T> tuple(alloc_tag, Alloc, T...) -> tuple<T...>;

template<class... T> using Tail = tuple<T..., int>;
Tail u{1.0f, 2};
