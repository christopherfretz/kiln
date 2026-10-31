// [expr.prim.lambda.closure]/1: the call operator's exception
// specification is the lambda-declarator's; the conversion function is
// noexcept.
auto a = []() noexcept { return 1; };
auto b = [] { return 1; };
static_assert(noexcept(a()));
static_assert(!noexcept(b()));
static_assert(noexcept(+b));
void (*pf)() noexcept = []() noexcept {};
