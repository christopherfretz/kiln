// [temp.param]/9, [dcl.type.auto.deduct]/2.4: a placeholder non-type
// template parameter deduces from its argument.
//@ strict-diags
template <auto N> struct C { static constexpr auto value = N; };
static_assert(__is_same(decltype(C<1>::value), const int));
static_assert(__is_same(decltype(C<'x'>::value), const char));
static_assert(__is_same(decltype(C<2u>::value), const unsigned));
static_assert(C<5>::value == 5);
template <decltype(auto) N> struct D { static constexpr decltype(N) value = N; };
static_assert(__is_same(decltype(D<3L>::value), const long));
