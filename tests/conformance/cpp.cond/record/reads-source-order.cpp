// [cpp.cond]/5: with `-fkiln-dump-pp-record=source-order` a condition's
// reads print in source order: the `__has_include` builtin before the
// macro its operand expanded, and `F` before the `A` of its argument
// (spec/pp.md "Preprocessing record", dump). The same reads in entry
// order are in reads-entry-order.record.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): a kiln dump option
//@ args: -fkiln-dump-pp-record=source-order
//@ pp-record: reads-source-order.record
#define HDR <no-such-header.h>
#define A 1
#define F(x) (x)
#if !__has_include(HDR) && F(A) && defined(A)
int a;
#endif
