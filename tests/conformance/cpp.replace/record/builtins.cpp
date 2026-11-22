// [cpp.predefined]: three expansions of builtin definitions, each result a
// made token of kind builtin (spec/pp.md "Preprocessing record").
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: builtins.record
int line = __LINE__;
const char *file = __FILE_NAME__;
int c0 = __COUNTER__;
