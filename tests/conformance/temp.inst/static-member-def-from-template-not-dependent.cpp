// [temp.inst]/4, [temp.dep.type]/1, [temp.dep.constexpr]/2: the definition
// of a static data member of a class template specialization, instantiated
// from the template's out-of-class definition, is a member of that
// (non-dependent) specialization, even when the reference that instantiates
// it is a non-dependent name in another template's definition. A later
// reference to the member in a template argument is then not
// instantiation-dependent once its template's arguments are substituted.
// The shape is boost.mpl's `apply2<add_alignment, size_t<1>, blank>` in
// boost::variant: `type_traits::integral_constant<T, val>::value` has an
// out-of-class definition, `type_with_alignment<Align>` names
// `alignment_of<char>::value` in its definition, and `add_alignment` folds
// `lcm<State::value, alignment_of<Item>::value>::value`.
//@ expect: run
//@ strict-diags
//@ exit: 3
template <class T, T val> struct integral_constant { static const T value = val; };
template <class T, T val> T const integral_constant<T, val>::value;
template <class T> struct alignment_of : integral_constant<unsigned long, __alignof(T)> {};
template <unsigned long Align> struct X {
  static const bool b = alignment_of<char>::value >= Align;
};
template <unsigned long A, unsigned long B> struct lcm { static const unsigned long value = A * B; };
template <unsigned long N> struct size_c { static const unsigned long value = N; };
template <typename State, typename Item>
struct apply : size_c<lcm<State::value, alignment_of<Item>::value>::value> {};
struct blank {};
int main() { return apply<size_c<3>, blank>::value; }
