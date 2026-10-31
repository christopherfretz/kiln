// [temp.arg.nontype]: pointer and nullptr non-type template arguments
// substitute back as address-of / null-pointer expressions.
//@ expect: compile-ok
int g;
int arr[4];
constexpr int f() { return 42; }
template <int* q> constexpr int* get() { return q; }
static_assert(get<&g>() == &g);
static_assert(get<nullptr>() == nullptr);
static_assert(get<arr>() == &arr[0]);
template <int (*fp)()> constexpr int call() { return fp(); }
static_assert(call<f>() == 42);
static_assert(call<&f>() == 42);
template <decltype(nullptr) n> constexpr bool is_null() { return n == nullptr; }
static_assert(is_null<nullptr>());
template <const int* p> struct W { static constexpr const int* v = p; };
static_assert(W<&g>::v == &g);
static_assert(__is_same(W<&g>, W<&g>));
static_assert(!__is_same(W<&g>, W<nullptr>));
