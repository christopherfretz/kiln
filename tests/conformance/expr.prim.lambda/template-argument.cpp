// [expr.prim.lambda.general]/2 (C++20): a lambda-expression may appear in a
// template argument and in a default template argument.
//@ xfail(kiln): calling a placeholder (auto) template parameter whose default is a closure gives "called object type 'auto'", and the parser does not take a lambda-expression as a template argument
template <auto F = [] { return 5; }> constexpr int get() { return F(); }
static_assert(get() == 5);
template <int N> struct K { static constexpr int value = N; };
static_assert(K<[] { return 3; }()>::value == 3);
