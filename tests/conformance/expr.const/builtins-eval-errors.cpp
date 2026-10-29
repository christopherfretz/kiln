// [expr.const]/5, [bit.count], [char.traits.require], [c.math.fpclass]: the
// builtins reject what is not a constant: a zero operand to clz/ctz, reads
// past the end of an array, memcpy beyond its source, reaching
// __builtin_unreachable; signbit and the ordered comparisons are not
// constant-evaluable at all (as in clang 19).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:19 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 23:19 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 25:16 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 25:37 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 26:15 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 26:19 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 28:15 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 27:53 note "'memcpy' not supported: source is not a contiguous array of at least 2 elements of type 'int'"
//@ diag: 28:19 note "in call to 'cp()'"
//@ diag: 30:15 error "constexpr variable 'f' must be initialized by a constant expression"
//@ diag: 29:44 note "subexpression not valid in a constant expression"
//@ diag: 30:19 note "in call to 'un(0)'"
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
constexpr int a = __builtin_clz(0u);
constexpr int b = __builtin_ctzll(0ull);
constexpr char nt[3] = {'a', 'b', 'c'};
constexpr auto c = __builtin_strlen(nt);
constexpr int d = __builtin_memcmp("ab", "ab", 5);
constexpr int cp() { int x[2] = {}; int y[1] = {1}; __builtin_memcpy(x, y, 8); return x[0]; }
constexpr int e = cp();
constexpr int un(int n) { if (n) return n; __builtin_unreachable(); }
constexpr int f = un(0);
static_assert(__builtin_signbit(-1.0));
static_assert(__builtin_isless(1.0, 2.0));
