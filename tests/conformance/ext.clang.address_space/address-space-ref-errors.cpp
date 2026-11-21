// Reference binding across address spaces (WP-1803): a reference binds
// only to an object in a space its referee's space holds, and a temporary
// lives in the generic space. clang wording, including the overload
// candidate notes that name the two spaces.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:22 error "no matching function for call to 'ref'"
//@ diag: 38:6 note "candidate function not viable: cannot bind reference in generic address space to object in address space '1' in 1st argument"
//@ diag: 41:21 error "no matching function for call to 'pf'"
//@ diag: 40:6 note "candidate function not viable: cannot pass pointer to generic address space as a pointer to address space '1' in 1st argument"
//@ diag: 43:55 error "no matching function for call to 'pf2'"
//@ diag: 42:6 note "candidate function not viable: cannot pass pointer to address space '2' as a pointer to generic address space in 1st argument"
//@ diag: 45:23 error "no matching function for call to 'cref'"
//@ diag: 44:6 note "candidate function not viable: cannot bind reference in generic address space to object in address space '1' in 1st argument"
//@ diag: 45:32 error "no matching function for call to 'cref'"
//@ diag: 44:6 note "candidate function not viable: cannot bind reference in generic address space to object in address space '1' in 1st argument"
//@ diag: 48:16 error "no matching function for call to 'gref'"
//@ diag: 46:6 note "candidate function not viable: cannot bind reference in address space '2' to object in generic address space in 1st argument"
//@ diag: 50:8 error "binding reference of type 'int' to value of type '__attribute__((address_space(2))) int' changes address space"
//@ diag: 51:14 error "binding reference of type 'const int' to value of type '__attribute__((address_space(2))) int' changes address space"
//@ diag: 52:48 error "binding reference of type 'const __attribute__((address_space(1))) int' to value of type 'int' changes address space"
//@ diag: 56:38 error "binding reference of type '__local int' to value of type '__generic int' changes address space"
//@ diag: 59:49 error "no matching function for call to 'a'"
//@ diag: 58:6 note "candidate function not viable: cannot pass pointer to address space '__local' as a pointer to address space '__global' in 1st argument"
//@ diag: 61:51 error "no matching function for call to 'b'"
//@ diag: 60:6 note "candidate function not viable: cannot pass pointer to address space '__private' as a pointer to generic address space in 1st argument"
//@ diag: 63:51 error "no matching function for call to 'c'"
//@ diag: 62:6 note "candidate function not viable: cannot bind reference in address space '__generic' to object in address space '__constant' in 1st argument"
//@ diag: 65:54 error "no matching function for call to 'd'"
//@ diag: 64:6 note "candidate function not viable: no known conversion from '__attribute__((address_space(1))) int **' to 'int **' for 1st argument"
//@ diag: 67:62 error "no matching function for call to 'e'"
//@ diag: 66:6 note "candidate function not viable: cannot pass pointer to address space '1' as a pointer to generic address space in 1st argument"
//@ diag: 74:19 error "cannot initialize object parameter of type 'const S' with an expression of type '__attribute__((address_space(1))) S'"
//@ diag: 75:22 error "no matching member function for call to 'g'"
//@ diag: 70:7 note "candidate function not viable: 'this' object is in address space '1', but method expects object in generic address space"
//@ diag: 71:7 note "candidate function not viable: requires 1 argument, but 0 were provided"

void ref(__attribute__((address_space(1))) int &r);
void uref() { int x; ref(x); }
void pf(__attribute__((address_space(1))) int *r);
void upf() { int x; pf(&x); }
void pf2(int *r);
void upf2(__attribute__((address_space(2))) int *p) { pf2(p); }
void cref(const __attribute__((address_space(1))) int &r);
void ucref() { int x; cref(x); cref(1); }
void gref(int &r);
extern __attribute__((address_space(2))) int gx;
void ugref() { gref(gx); }
void bind() {
  int &r = gx;
  const int &cr = gx;
  __attribute__((address_space(1))) const int &t = 3;
}
void gen(__attribute__((opencl_global)) int &gg) {
  __attribute__((opencl_generic)) int &r = gg;
  __attribute__((opencl_local)) int &l = r;
}
void a(__attribute__((opencl_global)) int *p);
void ua(__attribute__((opencl_local)) int *q) { a(q); }
void b(int *p);
void ub(__attribute__((opencl_private)) int *q) { b(q); }
void c(__attribute__((opencl_constant)) int &p);
void uc(__attribute__((opencl_generic)) int &q) { c(q); }
void d(int **p);
void ud(__attribute__((address_space(1))) int **q) { d(q); }
void e(const int *p);
void ue(volatile __attribute__((address_space(1))) int *q) { e(q); }
struct S {
  int f() const;
  int g();
  int g(int);
};
extern __attribute__((address_space(1))) S gs;
int uf() { return gs.f(); }
int ug() { return gs.g(); }
