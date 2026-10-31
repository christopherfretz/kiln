// [expr.prim.lambda.closure]/5 (P1169): a static lambda's call operator is
// a static member function; the conversion yields its address directly.
auto st = [](int x) static { return x + 1; };
static_assert(st(1) == 2);
constexpr int (*p)(int) = st;
static_assert(p(41) == 42);
static_assert(noexcept([]() static noexcept {}()));
constexpr int via_object() {
  auto l = [](int a, int b) static { return a * b; };
  return l(6, 7);
}
static_assert(via_object() == 42);
