// [class.temporary]/6, [basic.start.term]/1, [expr.const]/14 (WP-231): a
// namespace-scope reference bound to a temporary of class type with a
// non-trivial destructor is not constant-initialized (clang
// `CheckLValueConstantExpression`), so the `_ZGR` temporary is
// zero-initialized, built by the dynamic initializer, and destroyed at
// exit via `__cxa_atexit`.
struct D { int a; ~D(); };
const D& r = D{1};
struct E { int a; constexpr E(int x) : a(x) {} ~E() {} };
const E& r2 = E{2};
struct F { int a; int b; ~F(); };
F&& r3 = F{3, 4};
struct T { int a; };
const T& r4 = T{5};
int use() { return r.a + r2.a + r3.b + r4.a; }
