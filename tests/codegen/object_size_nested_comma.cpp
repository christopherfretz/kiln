// __builtin_object_size of an operand with nested commas in a runtime function
// (clang 19 folds it with tryEvaluateBuiltinObjectSize, [expr.comma]/1).
int ext();
struct S { int a; char c[8]; };
long r1() { char b[6] = {}; return __builtin_object_size(&b[(ext(), 1)], 0); }
long r2(int i) { char b[6] = {}; return __builtin_object_size(&b[(i++, 1)], 0) + i; }
long r3() { S s[2] = {}; return __builtin_object_size(&s[(ext(), 1)].c[(ext(), 2)], 1); }
long r4() { char b[6] = {}; char *p = b; return __builtin_object_size((ext(), p) + 1, 0); }
long r5() { char b[6] = {}; return __builtin_object_size(b + (ext(), 2), 2); }
