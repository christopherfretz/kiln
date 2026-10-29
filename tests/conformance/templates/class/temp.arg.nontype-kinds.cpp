//@ expect: ast
//@ oracle: clang
// [temp.arg.nontype]/1: bool, unsigned and negative non-type arguments are
// converted to the parameter's type.
template <bool B> struct Flag { static constexpr bool value = B; };
template <unsigned long N> struct Size { char c; };
template <int N> struct Signed { char c; };
static_assert(Flag<true>::value);
static_assert(!Flag<false>::value);
static_assert(sizeof(Size<18446744073709551615UL>) == 1);
static_assert(sizeof(Signed<-3>) == 1);
