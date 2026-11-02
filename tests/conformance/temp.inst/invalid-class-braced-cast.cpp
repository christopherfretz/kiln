// [temp.inst]/2, [expr.type.conv]: a variable template initialized with
// `ill<T>{}`, where instantiating `ill<T>` fails its static_assert, reports
// only the static_assert: the list-initialization of the invalid class is
// dropped without a "no viable conversion" (`std::numbers::e_v<int>`).
// An explicit specialization does not instantiate the primary.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:17 error "static assertion failed due to requirement 'always_false<int>': primary template instantiated"
//@ diag: 18:36 note "in instantiation of template class 'ill<int>' requested here"
//@ diag: 21:9 note "in instantiation of variable template specialization 'v<int>' requested here"
//@ diag: 16:17 error "static assertion failed due to requirement 'always_false<long>': primary template instantiated"
//@ diag: 18:36 note "in instantiation of template class 'ill<long>' requested here"
//@ diag: 22:10 note "in instantiation of variable template specialization 'v<long>' requested here"
template <class T> constexpr bool always_false = false;
template <class T> struct ill {
  static_assert(always_false<T>, "primary template instantiated");
};
template <class T> constexpr T v = ill<T>{};
template <> constexpr double v<double> = 1.0;
double d = v<double>;
int i = v<int>;
long l = v<long>;
