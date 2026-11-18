// [basic.link]/15-17 (WP-1684): a variable usable in constant expressions
// with internal linkage is TU-local, and /15.4 ignores only the references
// to it that are not odr-uses: taking its address or binding a reference
// to it in the body of an inline function (constexpr, a member defined in
// its class with constexpr, a lambda there, an inline member defined out
// of its class) exposes it. A variable whose type is a closure type
// exposes what the lambda's body names: the closure type is part of the
// variable's type, which /15.1.2 does not ignore, and its function call
// operator is inline. Reported at the function and at the lambda, as GCC
// 16.1 does (an out-of-class member at its name: GCC 16.1 at its
// qualifier); clang 22 accepts every one.
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 accepts
//@ strict-diags
//@ diag: 41:26 error "declaration of 'address_of_k' exposes TU-local entity 'K'"
//@ diag: 42:26 error "declaration of 'reference_to_k' exposes TU-local entity 'K'"
//@ diag: 43:29 error "declaration of 'constexpr_address' exposes TU-local entity 'K2'"
//@ diag: 44:19 error "declaration of 'module_linkage_address' exposes TU-local entity 'K'"
//@ diag: 45:20 error "declaration of 'by_reference' exposes TU-local entity 'K'"
//@ diag: 46:47 error "declaration of 'in_lambda' exposes TU-local entity 'K'"
//@ diag: 47:45 error "declaration of 'f' exposes TU-local entity 'K'"
//@ diag: 49:31 error "declaration of 'f' exposes TU-local entity 'K'"
//@ diag: 50:30 error "declaration of 'closure' exposes TU-local entity 'helper'"
//@ diag: 51:34 error "declaration of 'closure_not_inline' exposes TU-local entity 'helper'"
//@ diag: 52:38 error "declaration of 'closure_module_linkage' exposes TU-local entity 'helper'"
//@ diag: 53:43 error "declaration of 'closure_constexpr' exposes TU-local entity 'helper'"
//@ diag: 54:38 error "declaration of 'closure_generic' exposes TU-local entity 'helper'"
//@ diag: 55:38 error "declaration of 'closure_address' exposes TU-local entity 'K'"

export module M;
static int helper() { return 1; }
static const int K = 5;
static constexpr int K2 = 3;
static int hidden;
namespace {
struct S { int v; };
}
inline void take(const int &) {}

export inline const int *address_of_k() { return &K; }
export inline const int &reference_to_k() { return K; }
export constexpr const int *constexpr_address() { return &K2; }
inline const int *module_linkage_address() { return &K; }
export inline void by_reference() { take(K); }
export inline const int *in_lambda() { return [] { return &K; }(); }
export struct Member { constexpr const int *f() const { return &K; } };
export struct OutOfClass { const int *f(); };
inline const int *OutOfClass::f() { return &K; }
export inline auto closure = [] { return helper(); };
export auto closure_not_inline = [] { return helper(); };
inline auto closure_module_linkage = [] { return helper(); };
export constexpr auto closure_constexpr = [] { return helper(); };
export inline auto closure_generic = [](auto) { return helper(); };
export inline auto closure_address = [] { return &K; };
