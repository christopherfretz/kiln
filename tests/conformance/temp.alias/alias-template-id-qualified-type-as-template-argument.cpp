// [temp.alias]/2, [temp.arg.general]/2, [temp.res.general]/4: in a
// template-argument, `A<args>::m` with `A` an alias template naming a class
// template specialization is a type-id when `m` names a type: an
// alias-template-id is equivalent to the substituted type, so `m` is looked
// up in that class (libc++ range.elements/iterator/member_types:
// `std::same_as<ElementsIter<Range<I>>::iterator_concept, ...>`, with
// `ElementsIter` an alias of `iterator_t<elements_view<R, N>>`).
//@ expect: compile-ok
//@ strict-diags
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;

template <class T> struct it { using tag = T*; static constexpr int value = 1; };
template <class T> struct W {};
template <class I> using R = W<I>;
template <class Rg, int N = 0> using EI = it<Rg>;
template <class Rg> using EE = EI<Rg, 1>;
template <class Rg> using Ptr = Rg*;
// The pattern names no class template: the alias-template-id is resolved.
template <class T> T make();
template <class Rg> using DT = decltype(make<it<Rg>>());
template <int> struct I {};

template <class Q> struct S {};

static_assert(same<EI<R<int>>::tag, W<int>*>);
static_assert(same<EI<int>::tag, int*>);
static_assert(same<EE<long>::tag, long*>);
static_assert(EI<int>::value == 1);
S<EI<char>::tag> s1;
S<EE<char>::tag> s2;
S<Ptr<int>> s3;
static_assert(same<DT<int>::tag, int*>);
S<DT<R<int>>::tag> s4;
I<DT<int>::value> i1;

template <class T> void ok() {
  S<typename EI<T>::tag> s;
  static_assert(same<typename EE<T>::tag, T*>);
  static_assert(same<typename DT<T>::tag, T*>);
  I<DT<T>::value> i;
}

int main() { ok<int>(); }
