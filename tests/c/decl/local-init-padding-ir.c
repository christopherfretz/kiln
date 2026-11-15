// A C aggregate initialized from a list that is not a constant (clang
// 20+ `AggExprEmitter::DoZeroInitPadding`, C only): before each member
// the padding since the previous one is set to zero with a `memset` (on a
// byte GEP without `inbounds`), a bit-field's storage unit is zeroed by
// one integer store before its first field is stored, the tail padding
// last; a union's bytes past its member are zeroed, and an empty union
// list zeroes the whole union. clang 19 (KILN_ORACLE=19) writes only the
// members. Initialization stops at a flexible array member (all
// versions). C23 6.7.11p11; spec/codegen.md "Aggregate initialization".
//@ expect: ir
//@ args: -std=gnu17
struct S { char c; int i; };
struct B { char a; int b : 3, c : 9; char d; long e; };
union U { char c; long l; };
struct F { char c; int n; int d[]; };
struct N { struct S s; char z; };
void use(void *);
void f1(int x) { struct S l = {1, x}; use(&l); }
void f2(int x) { struct B b = {1, x, 2, 3, x}; use(&b); }
void f3(int x) { union U u = {x}; use(&u); }
void f4(int x) { union U u = {}; struct S s = {}; use(&u); use(&s); }
void f5(int x) { struct F f = {1, x}; use(&f); }
void f6(int x) { struct N n = {{1, x}, 2}; use(&n); }
void f7(int x) { struct S a[2] = {{1, x}}; use(a); }
void f8(int x) { struct S s; s = (struct S){x, 2}; use(&s); }
void f9(int x) { struct B b = {.d = x}; use(&b); }
