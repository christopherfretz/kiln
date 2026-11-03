// [expr.prim.lambda.closure]/13: only a closure type with no
// lambda-capture has a (defaulted) default constructor; a capturing
// closure has none, so it is neither trivial nor POD ([class.prop]/2,
// [basic.types.general]/9), though it stays trivially copyable.
//@ strict-diags
int g = 0;
auto a = [x = 3] { return x; };
auto b = [] { return 1; };
auto c = [&g2 = g] { return g2; };
static_assert(!__is_trivial(decltype(a)));
static_assert(!__is_pod(decltype(a)));
static_assert(!__is_trivially_constructible(decltype(a)));
static_assert(!__is_constructible(decltype(a)));
static_assert(__is_trivially_copyable(decltype(a)));
static_assert(__is_trivial(decltype(b)));
static_assert(__is_pod(decltype(b)));
static_assert(__is_trivially_constructible(decltype(b)));
static_assert(!__is_trivial(decltype(c)));
static_assert(!__is_pod(decltype(c)));
int f(int y) {
  auto d = [y] { return y; };
  static_assert(!__is_trivial(decltype(d)));
  return a() + b() + c() + d();
}
