// [expr.prim.lambda.general]/2 (C++20): a lambda-expression may appear in an
// unevaluated operand and in a template argument.
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
static_assert(sizeof([] { return 0; }) == 1);
static_assert(same<decltype([](int x) { return x; }(1)), int>);
static_assert(noexcept([]() noexcept {}));
template <auto F = [] { return 5; }> constexpr int get() { return F(); }
static_assert(get() == 5);
template <int N> struct K { static constexpr int value = N; };
static_assert(K<[] { return 3; }()>::value == 3);
