// [temp.arg.nontype]/1: a value-dependent argument of class type `q` for
// a class-type parameter `n` of an alias template must convert to `n`;
// the conversion is checked at the definition (WP-1497).
//@ expect: compile-fail
//@ std: c++20
//@ diag: 10:38 error "no viable conversion from 'const q' to"
struct n { constexpr n(int a) : i(a) {} int i; };
struct q { int z; };
template <n N> using i_type = decltype(N.i);
template <q X> using j_type = i_type<X>;
