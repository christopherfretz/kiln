// [temp.param]/5, [temp.arg.nontype]/1: the placeholder of
// `template<int N, C<N> auto V>` names an earlier parameter in its
// type-constraint; once the prior arguments are known it is no longer
// dependent, so an argument for `V` is deduced, checked against
// `C<decltype(V), N>` and evaluated, and two template-ids naming the same
// object are the same specialization (a call result and the variable it
// returns, also through a default argument).
//@ verify
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;

struct origin {};
inline constexpr origin zero;
consteval origin make() { return zero; }

template <class T, int N> concept Tagged = N == 7;

template <int N, Tagged<N> auto V> struct box { // #box
  static constexpr auto value = V;
};
static_assert(same<box<7, make()>, box<7, zero>>);
static_assert(same<decltype(box<7, make()>::value), const origin>);

template <class R> consteval int tag(R) { return 7; }
template <auto R, Tagged<tag(R)> auto V = make()> struct pt {};
static_assert(same<pt<1>, pt<1, zero>>);
static_assert(same<pt<1, make()>, pt<1>>);

box<6, make()> bad; // expected-error {{constraints not satisfied for class template 'box' [with N = 6, V = origin{}]}}
// expected-note@#box {{because 'Tagged<decltype(}}
// expected-note@-15 {{because '6 == 7' (6 == 7) evaluated to false}}
