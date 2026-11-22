// [cpp.stringize]: one stringized token from argument 0, whose span covers
// `L::T`; `x` is used only by `#` (spec/pp.md "Preprocessing record").
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: stringize.record
#define STR(x) #x
const char *s = STR(L::T);
