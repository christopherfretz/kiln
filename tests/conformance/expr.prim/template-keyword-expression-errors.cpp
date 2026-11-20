// [expr.prim]: no primary expression starts with `template` (but for a
// splice, [expr.prim.splice]): clang's "expected expression" at the
// keyword (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:9 error "expected expression"
//@ diag: 11:10 error "expected expression"

template<class T> struct A { template<class U> static int f(); };
int x = template A<int>::f<int>();
int y = (template f);
