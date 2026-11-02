// [temp.arg.nontype]/1, [temp.inst]: a floating-point template argument
// substituted for its parameter in a function template's body is the
// literal of its value (clang `BuildExpressionFromNonTypeTemplateArgument`),
// a prvalue of the parameter's type (libc++ `optional<T&>` tests' `T t{_Val}`).
//@ expect: compile-ok
//@ strict-diags
template <class T, T V> constexpr T get() { T t{V}; return t; }
static_assert(get<double, 15.0>() == 15.0);
static_assert(get<float, -2.5f>() == -2.5f);
template <class T, T V> constexpr bool same() { return __is_same(decltype(V), T); }
static_assert(same<double, 1.0>());
template <class T, __remove_reference_t(T) V> constexpr bool ref() {
  T t{V};
  T& r = t;
  return r == V;
}
static_assert(ref<double, 1.5>());
