// [cpp.concat]: `L_init` is pasted from argument 0 and the body token
// `_init`; `ns` has no plain use, so no delivered token has an argument
// origin (spec/pp.md "Preprocessing record").
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: paste.record
void L_init();
#define INIT(ns) ns##_init()
void f() { INIT(L); }
