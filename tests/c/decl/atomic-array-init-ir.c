// A local array of `_Atomic` elements is initialized element by element, not
// copied from a constant: clang's `isPODType` is false for an atomic type, so
// the array is no constant aggregate (`IsConstantAggregate` in
// `EmitAutoVarInit`); a mostly zero list is a `memset` and the explicit
// elements, a long zero tail a filler loop. A structure with an `_Atomic`
// member is still copied from a constant. C23 6.7.11; spec/codegen.md "C".
//@ expect: ir
//@ args: -std=gnu17
void use(void *);
struct Q { _Atomic int a; int b; };
void f(void) { _Atomic int a[4] = {1, 2, 3, 4}; use(a); }
void g(void) { _Atomic int a[8] = {1, 2}; use(a); }
void h(void) { _Atomic _Bool a[2] = {1, 0}; use(a); }
void k(void) { _Atomic long a[3] = {0}; use(a); }
void l(void) { _Atomic int a[20] = {1, 2, 3}; use(a); }
void m(void) { _Atomic double a[2] = {1.5, 2}; use(a); }
void n(void) { _Atomic int a[2][2] = {{1, 2}, {3}}; use(a); }
void p(void) { struct Q q = {1, 2}; use(&q); }
void r(void) { struct Q a[2] = {{1, 2}}; use(a); }
