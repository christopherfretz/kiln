// [cpp.pragma.op], [cpp.pragma]: five `_Pragma("push_macro(...)")` in one
// macro body, then five pops, as a library header wraps itself. Each
// event is keyed by its `_Pragma`'s expansion; the pop of `min` names
// its push and restores the definition pushed, by the same key; the
// definition it ended is "popped" at the same key (spec/pp.md
// "Preprocessing record", push and pop).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: pragma-push.record
#define PUSH _Pragma("push_macro(\"min\")") _Pragma("push_macro(\"max\")") \
  _Pragma("push_macro(\"a\")") _Pragma("push_macro(\"b\")") _Pragma("push_macro(\"c\")")
#define POP _Pragma("pop_macro(\"min\")") _Pragma("pop_macro(\"max\")") \
  _Pragma("pop_macro(\"a\")") _Pragma("pop_macro(\"b\")") _Pragma("pop_macro(\"c\")")
#define min 1
PUSH
#undef min
#define min 2
int x = min;
POP
int m = min;
