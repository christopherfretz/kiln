// [expr.add]/1, /2.3: a pointer difference with a void or function pointee
// on one side is diagnosed for that side, on both as 'pointers to'; two
// different function pointees are named together; then an incomplete
// pointee, left first; an incompatible pair is diagnosed first (clang
// checkArithmeticBinOpPointerOperands, CheckSubtractionOperands; WP-230)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:14 error "arithmetic on pointers to the function type 'void ()'"
//@ diag: 32:14 error "'void (*)()' and 'void (*)(int)' are not pointers to compatible types"
//@ diag: 32:14 error "arithmetic on pointers to function types 'void ()' and 'void (int)'"
//@ diag: 33:14 error "arithmetic on a pointer to the function type 'void ()'"
//@ diag: 35:13 error "arithmetic on pointers to void"
//@ diag: 36:13 error "arithmetic on a pointer to void"
//@ diag: 38:14 error "arithmetic on pointers to the function type 'void ()'"
//@ diag: 39:14 error "arithmetic on a pointer to the function type 'void ()'"
//@ diag: 39:19 error "arithmetic on a pointer to void"
//@ diag: 41:14 error "'int *' and 'void *' are not pointers to compatible types"
//@ diag: 41:14 error "arithmetic on a pointer to void"
//@ diag: 42:13 error "'void *' and 'int *' are not pointers to compatible types"
//@ diag: 42:13 error "arithmetic on a pointer to void"
//@ diag: 43:14 error "arithmetic on a pointer to an incomplete type 'S'"
//@ diag: 40:17 note "forward declaration of 'S'"
//@ diag: 44:14 error "'int *' and 'S *' are not pointers to compatible types"
//@ diag: 44:14 error "arithmetic on a pointer to an incomplete type 'S'"
//@ diag: 40:17 note "forward declaration of 'S'"
//@ diag: 45:14 error "'S *' and 'int *' are not pointers to compatible types"
//@ diag: 45:14 error "arithmetic on a pointer to an incomplete type 'S'"
//@ diag: 40:17 note "forward declaration of 'S'"
//@ diag: 46:14 error "arithmetic on pointers to void"
void f(); void g(int);
auto d1 = &f - &f;
auto d2 = &f - &g;
auto d3 = &f + 1;
void *p, *q;
auto d4 = p - q;
auto d5 = p + 1;
void (*fp)() = f;
auto d6 = fp - fp;
void h() { fp++; p++; }
int *ip; struct S; S *sp; const void *cp;
auto e1 = ip - p;
auto e2 = p - ip;
auto e3 = sp - sp;
auto e4 = ip - sp;
auto e5 = sp - ip;
auto e6 = cp - p;
auto e7 = fp - p;
auto e8 = &g - ip;
auto e9 = ip - &g;
