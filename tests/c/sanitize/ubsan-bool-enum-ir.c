// -fsanitize=bool,enum in C (clang `EmitScalarRangeCheck`): a `_Bool`
// load, a `_Bool` bit-field wider than one bit's value and a load through
// a cast to `_Bool *` are checked; an enumeration load never is, since
// any value of the compatible integer type is a value of the enumerated
// type (C17 6.7.2.2p4; clang's `getRangeForType` gives only a C++
// enumeration without a fixed underlying type a range). The handler's
// type descriptor names the type as C prints it: `'_Bool'`.
// spec/codegen.md "Sanitizers".
//@ expect: ir
//@ args: -std=gnu17 -fsanitize=bool,enum
enum E { A, B };
struct S { _Bool b; enum E e; _Bool bf : 1; enum E ebf : 3; unsigned ub : 1; };
extern _Bool g;
int f1(enum E *p) { return *p; }
_Bool f2(_Bool *p) { return *p; }
int f3(struct S *s) { return s->b + s->e + s->bf + s->ebf + s->ub; }
int f4(void *p) { return *(enum E *)p + *(_Bool *)p; }
_Bool f5(void) { return g; }
int f6(struct S *s, int i) { return s[i].b; }
