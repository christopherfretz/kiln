// [expr.prim.lambda.closure]/5, [dcl.constexpr]/3 (C++20): a lambda not
// declared constexpr has a constexpr call operator only if it satisfies the
// requirements of a constexpr function, so before C++23 one that defines a
// variable of non-literal type is silently not constexpr: in a template
// checked for a variable of non-dependent type, and again when the
// enclosing template is instantiated; a generic lambda's specialization
// follows its pattern. clang 19 and clang 22 agree. WP-414.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:15 note "non-constexpr function 'operator()' cannot be used in a constant expression"
//@ diag: 28:20 note "declared here"
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:15 note "non-constexpr function 'operator()<int>' cannot be used in a constant expression"
//@ diag: 30:20 note "declared here"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:46 note "non-constexpr function 'operator()' cannot be used in a constant expression"
//@ diag: 34:15 note "in call to 't<V>()'"
//@ diag: 33:46 note "declared here"
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:46 note "non-constexpr function 'operator()' cannot be used in a constant expression"
//@ diag: 37:15 note "in call to 'u<int>()'"
//@ diag: 36:46 note "declared here"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 41:17 note "non-literal type 'V' cannot be used in a constant expression"
struct V { volatile int m = 0; constexpr V() = default; };
constexpr auto l = [] { V x; return 0; };
static_assert(l() == 0);
constexpr auto a = [](auto) { V x; return 0; };
static_assert(a(1) == 0);
// Checked again when the enclosing template is instantiated.
template<class T> constexpr int t() { return [] { T x; return 0; }(); }
static_assert(t<V>() == 0);
static_assert(t<int>() == 0);
template<class T> constexpr int u() { return [] { V x; return 0; }(); }
static_assert(u<int>() == 0);
// A generic lambda's specialization is not checked: it fails when the
// object is created.
constexpr auto b = [](auto y) { decltype(y) x; return 0; };
static_assert(b(V()) == 0);
// A lambda that is not called in a constant expression is fine.
int run = l() + a(1) + b(V());
