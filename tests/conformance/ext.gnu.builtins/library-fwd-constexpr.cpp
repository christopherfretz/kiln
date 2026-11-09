// The `__builtin_` library aliases (WP-1104) are not constant
// expressions ([expr.const]/10: a call of a function that is not
// constexpr): the Oven rejects them with clang's diagnostic instead of
// evaluating them (GNU [dcl.attr] extension).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:21 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 16:15 error "static assertion expression is not an integral constant expression"
//@ diag: 15:29 note "subexpression not valid in a constant expression"
//@ diag: 16:15 note "in call to 'f()'"
//@ diag: 18:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:29 note "subexpression not valid in a constant expression"
//@ diag: 18:15 note "in call to 'g()'"
constexpr void *p = __builtin_malloc(4);
constexpr bool f() { return __builtin_strcpy(nullptr, nullptr) == nullptr; }
static_assert(f());
constexpr bool g() { return __builtin_calloc(1, 1) == nullptr; }
static_assert(g());
