// [expr.prim.lambda.closure]/8-10: a captureless non-generic lambda
// converts to a pointer to function; the conversion function is constexpr
// and the pointer, called, has the same effect as the call operator.
constexpr auto sq = [](int x) { return x * x; };
constexpr int (*fp)(int) = sq;
static_assert(fp(3) == 9);
using F = int (*)();
constexpr F g = [] { return 3; };
static_assert(g() == 3);
static_assert(+[] { return 1; } != nullptr);
constexpr int apply(int (*f)(int), int v) { return f(v); }
static_assert(apply([](int x) { return x + 10; }, 5) == 15);
static_assert(noexcept(static_cast<int (*)(int)>(sq)));
