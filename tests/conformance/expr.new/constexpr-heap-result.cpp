// [expr.const]/5.20, [expr.const] (permitted results): the value of a constant
// expression must not point to, or into, a heap allocation, which is also a
// leak when the evaluation ends. The result note is at the variable.
// Mirrors crates/kiln_oven/tests/new_delete.rs `expr_const_heap_result`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:32 note "allocation performed here was not deallocated"
//@ diag: 20:16 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 20:16 note "pointer to heap-allocated object is not a constant expression"
//@ diag: 20:20 note "heap allocation performed here"
//@ diag: 26:16 error "constexpr variable 'q' must be initialized by a constant expression"
//@ diag: 26:16 note "pointer to heap-allocated object is not a constant expression"
//@ diag: 26:24 note "heap allocation performed here"
//@ diag: 29:16 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 29:16 note "pointer to subobject of heap-allocated object is not a constant expression"
//@ diag: 28:31 note "heap allocation performed here"
constexpr int *make() { return new int(3); }
static_assert(make());
constexpr int *p = new int(1);

struct Pair {
  int a;
  int *b;
};
constexpr Pair q = {1, new int(2)};

constexpr int *sub() { return new int[2]{} + 1; }
constexpr int *r = sub();
