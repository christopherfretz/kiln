// [cpp.subst]: a declaring macro with two parameters. `namespace` is body
// token 0, `L` argument 0 and `Buf` argument 1, each argument with its
// file range (spec/pp.md "Preprocessing record").
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: declare.record
#define DECLARE(ns, T) namespace ns { struct T; }
DECLARE(L, Buf)
L::Buf *p;
