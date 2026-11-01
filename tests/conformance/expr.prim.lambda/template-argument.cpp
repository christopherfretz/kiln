// [expr.prim.lambda.general]/2 (C++20): a lambda-expression may appear in a
// template argument and in a default template argument.
template <auto F = [] { return 5; }> constexpr int get() { return F(); }
static_assert(get() == 5);
template <int N> struct K { static constexpr int value = N; };
static_assert(K<[] { return 3; }()>::value == 3);
