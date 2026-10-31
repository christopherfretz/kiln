// [expr.prim.lambda.closure]/1: each lambda-expression has a unique,
// unnamed closure type, even in an unevaluated operand (C++20).
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
auto a = [] {};
auto b = [] {};
static_assert(!same<decltype(a), decltype(b)>);
static_assert(same<decltype(a), decltype(a)>);
using X = decltype([] {});
using Y = decltype([] {});
static_assert(!same<X, Y>);
static_assert(sizeof(a) == 1);
static_assert(__is_empty(decltype(a)));
static_assert(__is_class(decltype(a)));
static_assert(!__is_union(decltype(a)));
static_assert(__is_trivially_copyable(decltype(a)));
