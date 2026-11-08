// GNU `cleanup(f)` checks in clang 22's order (WP-807; clang
// `handleCleanupAttr`, `ActOnCleanupAttr`): the subject (a local
// variable, else a warning), one argument, a function named by it (a
// qualified name or template-id with a -Wgcc-compat warning), one
// parameter; the parameter type is checked against `T *` once the
// variable's type is deduced, at instantiation for a dependent one, and
// may drop qualifiers. (clang 22's -Walign-mismatch on the fake call is
// not modelled: no case here has a misaligned parameter.) The clang 19
// behaviour is in cleanup-diags-clang19.cpp.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 61:32 error "'cleanup' function 'f2' must take 1 parameter"
//@ diag: 62:32 error "'cleanup' function 'f3' must take 1 parameter"
//@ diag: 65:32 error "'cleanup' argument 'g' is not a function"
//@ diag: 66:32 error "'cleanup' argument 'ov' is not a single function"
//@ diag: 53:6 note "candidate function"
//@ diag: 52:6 note "candidate function"
//@ diag: 67:32 error "'cleanup' argument 'tf' is not a single function"
//@ diag: 54:25 note "candidate function template"
//@ diag: 68:32 warning "GCC does not allow the 'cleanup' attribute argument to be anything other than a simple identifier"
//@ diag: 69:32 warning "GCC does not allow the 'cleanup' attribute argument to be anything other than a simple identifier"
//@ diag: 70:32 error "'cleanup' function 'f7' parameter has type 'int' which is incompatible with type 'int *'"
//@ diag: 72:32 error "use of undeclared identifier 'undeclared'"
//@ diag: 73:24 error "'cleanup' attribute takes one argument"
//@ diag: 74:24 error "'cleanup' attribute takes one argument"
//@ diag: 75:32 error "'cleanup' argument is not a function"
//@ diag: 76:31 warning "'cleanup' attribute only applies to local variables"
//@ diag: 79:25 warning "'cleanup' attribute only applies to local variables"
//@ diag: 80:30 warning "'cleanup' attribute only applies to local variables"
//@ diag: 89:32 error "'cleanup' function 'fl' parameter has type 'long' which is incompatible with type 'int *'"
//@ diag: 90:30 error "'cleanup' function 'fbase' parameter has type 'B *' which is incompatible with type 'D *'"
//@ diag: 92:32 error "'cleanup' function 'fcr' parameter has type 'int *const &' which is incompatible with type 'int *'"
//@ diag: 97:30 error "'cleanup' function 'fd' parameter has type 'D *' which is incompatible with type 'B *'"
//@ diag: 100:33 error "'cleanup' function 'fll' parameter has type 'long long *' which is incompatible with type 'long *'"
//@ diag: 102:32 error "'cleanup' function 'fun' must take 1 parameter"
//@ diag: 104:33 error "'cleanup' function 'f1' parameter has type 'int *' which is incompatible with type 'int &*'"
//@ diag: 106:32 error "'cleanup' argument 'lam' is not a function"
//@ diag: 108:59 error "'cleanup' function 'f1' parameter has type 'int *' which is incompatible with type 'long *'"
//@ diag: 109:15 note "in instantiation of function template specialization 'tt<long>' requested here"
//@ diag: 118:30 warning "GCC does not allow the 'cleanup' attribute argument to be anything other than a simple identifier"
//@ diag: 119:32 error "'cleanup' argument is not a function"
//@ diag: 121:32 error "'cleanup' argument is not a function"
void f1(int *);
void f2(int *, int);
void f3();
void f4(long *);
void f5(const int *);
void f6(void *);
int g;
void ov(int *);
void ov(long *);
template <class T> void tf(T *);
struct S { void m(int *); static void sm(int *); };
void f7(int);
void f8(int **);
int f9(int *);
void test() {
  int a __attribute__((cleanup(f1)));
  int b __attribute__((cleanup(f2)));
  int c __attribute__((cleanup(f3)));
  int e __attribute__((cleanup(f5)));
  int h __attribute__((cleanup(f6)));
  int i __attribute__((cleanup(g)));
  int j __attribute__((cleanup(ov)));
  int k __attribute__((cleanup(tf)));
  int l __attribute__((cleanup(tf<int>)));
  int m __attribute__((cleanup(S::sm)));
  int n __attribute__((cleanup(f7)));
  int p __attribute__((cleanup(f9)));
  int q __attribute__((cleanup(undeclared)));
  int r __attribute__((cleanup));
  int s __attribute__((cleanup(f1, f1)));
  int t __attribute__((cleanup(1)));
  static int u __attribute__((cleanup(f1)));
  int v [[gnu::cleanup(f1)]];
}
int glob __attribute__((cleanup(f1)));
void fp(int x __attribute__((cleanup(f1))));
struct B {}; struct D : B {};
void fb(bool); void fl(long); void fbase(B *); void fref(int *&); void fcr(int *const &);
void f1(int *); void fv(void *); void fcv(const void *); void fvol(volatile int *);
void fd(D *); enum E { e0 }; void fe(E *);
typedef int I; void fi(I *); void fll(long long *);
void fa(int (*)[3]); void fun(...); template <class T> void tf(T *);
void test2() {
  int a __attribute__((cleanup(fb)));
  int b __attribute__((cleanup(fl)));
  D c __attribute__((cleanup(fbase)));
  int d __attribute__((cleanup(fref)));
  int e __attribute__((cleanup(fcr)));
  const int f __attribute__((cleanup(f1))) = 0;
  const int g __attribute__((cleanup(fv))) = 0;
  const int h __attribute__((cleanup(fcv))) = 0;
  int i __attribute__((cleanup(fvol)));
  B j __attribute__((cleanup(fd)));
  E k __attribute__((cleanup(fe)));
  int l __attribute__((cleanup(fi)));
  long m __attribute__((cleanup(fll)));
  int n[3] __attribute__((cleanup(fa)));
  int o __attribute__((cleanup(fun)));
  int &r = a; 
  int &p __attribute__((cleanup(f1))) = a;
  auto lam = [](int *) {};
  int q __attribute__((cleanup(lam)));
}
template <class T> void tt() { T x __attribute__((cleanup(f1))); }
template void tt<long>();
void f(int*);
void g2(int**);
void k(int *const*);
struct A { static void m(A*); };
void u() {
  auto x __attribute__((cleanup(f))) = 1;
  int *p __attribute__((cleanup(g2))) = 0;
  int *q __attribute__((cleanup(k))) = 0;
  A a __attribute__((cleanup(A::m)));
  int y __attribute__((cleanup((f))));
  int z [[gnu::cleanup(f)]];
  int w __attribute__((cleanup(&f)));
}
