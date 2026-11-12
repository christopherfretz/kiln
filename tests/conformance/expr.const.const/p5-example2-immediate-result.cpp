// [expr.const.const]/5 (P4101R1) Example 2: `constexpr auto e = g();`.
// The example's comment still calls it an error ("a pointer to an
// immediate function is not a permitted result of a constant
// expression"), but the normative text no longer supports that: /4 makes
// every core constant expression a constant expression unless its result
// holds an invalid pointer or an indeterminate or erroneous value (the
// permitted-result rule that excluded pointers to immediate functions
// was struck by P4101R1), `g()` is in an immediate function context as
// the initializer of a constexpr variable is manifestly
// constant-evaluated ([expr.const.imm]/1.2), and the object it
// initializes, holding a consteval-only value (/1.2), is an immediate
// object associated with a constexpr variable, which /3.1 allows (as for
// `constexpr auto b = plus1;` of /3 Example 1). So `e` is accepted and
// usable in constant expressions only; using it at run time is an error
// (/3, [expr.const.imm]/2.1). kiln only: GCC 16.1 rejects `e` under the
// earlier rule (docs/status/deviations.md).
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang implements the pre-P4101R1 permitted-result rule
//@ strict-diags
//@ diag: 30:20 error "consteval-only value outside an immediate function context"
consteval int f() { return 42; }
consteval auto g() { return f; }

constexpr auto e = g(); // OK (see above)
static_assert(e() == 42);
template <auto P> constexpr int call = P();
static_assert(call<e> == 42);

int run() { return e(); } // error: `e` refers to an immediate object
