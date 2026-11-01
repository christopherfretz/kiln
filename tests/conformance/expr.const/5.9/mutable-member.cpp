// [expr.const]/4, /5.9: a mutable subobject of an object usable in constant
// expressions is not itself usable, so reading it (or anything inside it)
// from an object whose lifetime began outside the evaluation is not a core
// constant expression; its non-mutable members still are.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:15 note "read of mutable member 'm' is not allowed in a constant expression"
//@ diag: 16:31 note "declared here"
//@ diag: 22:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:15 note "read of mutable member 'm' is not allowed in a constant expression"
//@ diag: 16:31 note "declared here"
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 25:15 note "read of mutable member 'i' is not allowed in a constant expression"
//@ diag: 18:22 note "declared here"
struct S { int k; mutable int m; };
struct I { int a; };
struct O { mutable I i; int z; };
constexpr S s{1, 2};
static_assert(s.m == 2);
constexpr const S *p = &s;
static_assert(p->m == 2);
static_assert(s.k == 1 && p->k == 1);
constexpr O o{{5}, 6};
static_assert(o.i.a == 5);
static_assert(o.z == 6);
