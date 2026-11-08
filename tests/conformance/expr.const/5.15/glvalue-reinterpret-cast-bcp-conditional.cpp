// [expr.const]/5.15: a glvalue `reinterpret_cast` is not a core constant
// expression. Under a conditional whose condition is
// `__builtin_constant_p(...)` (a GNU extension clang implements, GCC
// PR38377) the conditional is folded, and the cast does not fail it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:20 error "constexpr variable 'g' must be initialized by a constant expression"
//@ diag: 12:24 note "reinterpret_cast is not allowed in a constant expression"
struct Tag {};
struct L { int v; };
constexpr Tag tag{};
constexpr const L& g = reinterpret_cast<const L&>(tag);
constexpr const L& k = __builtin_constant_p(g) ? reinterpret_cast<const L&>(tag)
                                               : reinterpret_cast<const L&>(tag);
static_assert(&k == &k);
