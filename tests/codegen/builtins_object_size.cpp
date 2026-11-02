// GNU __builtin_object_size / __builtin_dynamic_object_size (clang
// emitBuiltinObjectSize at -O0): a pointer whose object is not known to
// the frontend lowers to `llvm.objectsize(ptr, min, nullunknown=true,
// dynamic)` with min = type & 2; type 3 of an unknown object folds to 0.
unsigned long os(char *p, int *q) {
  return __builtin_object_size(p, 0) + __builtin_object_size(q, 3) + __builtin_dynamic_object_size(p, 1);
}
unsigned long os2(void *p) { return __builtin_object_size(p, 2) + __builtin_dynamic_object_size(p, 3); }
// WP-292: objects the frontend knows fold (clang tryEvaluateObjectSize),
// pointer casts are looked through, a failing comma operand is ignored,
// and an operand with side effects (a call included) that does not fold
// gives the default without being emitted.
struct S { int a; char c[8]; };
unsigned long r1() { int buf[10]; return __builtin_object_size(buf, 0); }
unsigned long r2() { S s; return __builtin_object_size(&s.c[2], 1) + __builtin_object_size(&s.c[2], 0); }
unsigned long r3() { S s; return __builtin_object_size((const char *)&s, 1) + __builtin_object_size((char *)&s.a, 3); }
unsigned long r4() { int a[4]; int *p = a; return __builtin_object_size(p++, 0) + __builtin_object_size(p, 0); }
unsigned long r5() { int a[4]; return __builtin_object_size(a + 1, 0); }
static S g;
unsigned long r6() { return __builtin_object_size(&g.c[1], 1) + __builtin_dynamic_object_size(&g, 0); }
unsigned long r7(int i) { int a[4]; return __builtin_object_size(&a[i], 0); }
int ext();
unsigned long r8() { int a[4]; int i = 0; return __builtin_object_size((i++, a + 1), 0) + i; }
unsigned long r9() { char b[6]; return __builtin_object_size((ext(), b), 0); }
unsigned long r10() { char b[6]; return __builtin_object_size(&b[ext()], 0); }
unsigned long r11() { S s; return __builtin_object_size((void *)(char *)&s.c[3], 1); }
unsigned long r12() { S s; char *p = s.c; return __builtin_object_size(p++, 1) + __builtin_object_size(p, 2); }
unsigned long r13(char *p) { return __builtin_object_size((ext(), p), 0); }
unsigned long r14(char *p) { return __builtin_object_size(&p[ext()], 2); }
unsigned long r15(char *p, int i) { return __builtin_object_size(&p[i], 1); }
