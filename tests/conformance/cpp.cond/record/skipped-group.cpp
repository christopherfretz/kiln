// [cpp.cond]/6: a group whose condition is false is skipped. Nothing in it
// is expanded or defined; the `#if` is not taken and its group runs to the
// `#` of its `#endif`; the `#ifdef` nested in it is dead (nesting only,
// never evaluated) (spec/pp.md "Preprocessing record", conditionals).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: skipped-group.record
#if 0
LOG(L::x)
#define Z 1
#ifdef Q
#endif
#endif
int z;
