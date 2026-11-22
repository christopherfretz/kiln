// [cpp.cond]/10, /13: the reads of an evaluated condition, in order: the
// names tested by `defined` (a short-circuited operand too), each
// expansion, and each identifier left after expansion (it evaluates to
// 0), each with the definition bound or "undefined". A `defined` produced
// by a macro body is read at a macro location (spec/pp.md "Preprocessing
// record", condition reads).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: reads.record
//@ diag: 23:5 warning "macro expansion producing 'defined' has undefined behavior"
//@ diag: 22:11 note "expanded from macro 'D'"
#if defined(A) && B > 2
#error taken
#endif
#if 0 && defined(C)
#error taken
#endif
#define F(x) x
#if F(G) + H
#error taken
#endif
#define D defined(E)
#if D
#error taken
#endif
#define FN(x) x
#if FN || true
int fn;
#endif
