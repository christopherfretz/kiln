// [expr.prim.lambda.closure]/13 (C++20): the closure type of a lambda with
// no lambda-capture has a defaulted default constructor and a defaulted
// copy assignment operator.
auto a = [](int x) { return x + 1; };
decltype(a) b;
int use() {
  decltype(a) c{};
  c = b;
  return c(1);
}
static_assert(__is_trivially_copyable(decltype(a)));
static_assert(decltype(a){}(2) == 3);
