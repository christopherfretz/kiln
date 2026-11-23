// [cpp.cond]/5: by default a condition's reads print in entry order:
// the `__has_include` builtin after the macro its operand expanded, `F`
// before the `A` of its argument (spec/pp.md "Preprocessing record",
// dump). The same reads in source order: reads-source-order.cpp.
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: reads-entry-order.record
#define HDR <no-such-header.h>
#define A 1
#define F(x) (x)
#if !__has_include(HDR) && F(A) && defined(A)
int a;
#endif
