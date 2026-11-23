// [cpp.concat], [cpp.rescan]: a paste whose operands are arguments that
// came from another expansion (`XCAT` forwards them to `CAT`), a paste
// whose operand is itself a pasted token (`CAT(p, q)` pre-expanded in
// `XCAT`'s argument), and a paste whose left operand is a stringized
// token of the same expansion. Each operand's origin leads back to the
// file or to the made token (spec/pp.md "Preprocessing record", made
// tokens).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: paste-forwarded.record
#define CAT(a, b) a ## b
#define XCAT(a, b) CAT(a, b)
#define UDL(x) #x ## _s
long operator""_s(const char *, decltype(sizeof 0));
int XCAT(u, v);
int XCAT(CAT(p, q), r);
long s = UDL(w);
