// [expr.prim.lambda.closure]/9: the closure type of a generic lambda with
// no captures has a conversion function template to pointer to function;
// the target type deduces its arguments, and the invoker forwards to the
// call operator specialization for them. Usable in constant expressions.
constexpr auto id = [](auto x) { return x; };
constexpr int apply(int (*f)(int), int x) { return f(x); }
static_assert(apply(id, 3) == 3);
constexpr int (*a)(int) = id;
constexpr double (*b)(double) = id;
static_assert(a(4) == 4 && b(2.5) == 2.5);
constexpr auto two = [](auto x, auto y) { return x - y; };
constexpr long (*c)(long, int) = two;
static_assert(c(10, 3) == 7);
constexpr auto ref = [](const auto& x) -> const auto& { return x; };
constexpr int g = 9;
static_assert(&ref(g) == &g);
constexpr auto def = [](auto x, int y = 4) { return x + y; };
static_assert(def(1) == 5);
constexpr auto s = [](auto x) static { return x + 1; };
static_assert(s(1) == 2);
constexpr int (*sp)(int) = s;
static_assert(sp(2) == 3);
