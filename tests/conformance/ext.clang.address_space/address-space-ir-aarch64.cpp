// address_space(N) and opencl_* in the IR on aarch64 (WP-1803): globals and
// static locals in the space, `ptr addrspace(N)` loads, stores, GEPs and
// compares, `addrspacecast` for the C-style casts and the implicit
// conversions into the generic space (folded for constants), reference
// parameters and loads in a nonzero space without nonnull, and the
// Itanium mangling `U3AS<N>` / `U8CLglobal` with substitutions.
// The OpenCL spaces are LLVM address space 0 on this target.
//@ expect: ir
//@ args: --target=aarch64-linux-gnu -ftrivial-auto-var-init=uninitialized
#define AS1 __attribute__((address_space(1)))
#define AS2 __attribute__((address_space(2)))
AS1 int g = 3;
AS1 int *gp = &g;
int *gq = (int *)&g;
AS2 const int cg = 5;
AS1 int arr[4] = {1, 2, 3, 4};
extern AS1 int ex;
struct S { int a; int b; };
AS1 S gs = {1, 2};
int rd(AS1 int *p) { return *p; }
void wr(AS1 int *p, int v) { *p = v; p[2] = v; }
int *to0(AS1 int *p) { return (int *)p; }
AS1 int *to1(int *p) { return (AS1 int *)p; }
AS2 int *to2(AS1 int *p) { return (AS2 int *)p; }
int rdg() { return g + cg + arr[2] + ex; }
int mem() { return gs.b; }
AS1 int *memp() { return &gs.b; }
int ref(AS1 int &r) { return r; }
int callref() { return ref(g); }
bool cmp(AS1 int *a, AS1 int *b) { return a == b; }
bool isnull(AS1 int *a) { return a == nullptr; }
AS1 int *nul() { return nullptr; }
long diff(AS1 int *a, AS1 int *b) { return a - b; }
AS1 int *inc(AS1 int *a) { return a + 1; }
AS1 char *bc(AS1 int *a) { return (AS1 char *)a; }
AS1 void *vp(AS1 int *a) { return a; }
__attribute__((opencl_global)) int og = 1;
__attribute__((opencl_generic)) int *gen(__attribute__((opencl_global)) int *p) { return p; }
int *ocl(__attribute__((opencl_local)) int *p) { return (int *)p; }
long pint(AS1 int *p) { return (long)p; }
AS1 int *ipt(long v) { return (AS1 int *)v; }
bool tobool(AS1 int *p) { return p; }
int *const AS1 cpg = nullptr;
int sel(bool b, AS1 int *x, AS1 int *y) { return *(b ? x : y); }
AS1 int *decay() { return arr; }
AS1 int &elem(int i) { return arr[i]; }
int sum() { int s = 0; for (AS1 int &x : arr) s += x; return s; }
template <class T> T *id(T *p) { return p; }
AS1 int *useid(AS1 int *p) { return id(p); }
template <class T> struct Box { T *p; T get() const { return *p; } };
int boxget(Box<AS1 int> &b) { return b.get(); }
int local() { static AS1 int s = 7; return s++; }
struct T { int a; int f() const { return a; } };
extern AS1 T gt;
int call() { return ((T *)&gt)->f(); }
AS1 const int &cref() { static AS1 const int k = 9; return k; }
void store(AS1 int *p) { *p += 2; ++*p; }
AS1 int *const cp = &arr[1];
int rcp() { return *cp; }
void mf(AS1 int*, AS1 int*, AS1 const int*, AS1 const int*, int*) {}
void mg(AS1 int**, int * AS1 *, AS1 int&) {}
struct E {};
void mh(AS1 E*, AS1 E*, E*, AS1 volatile E*) {}
template <class T> void tt(T) {}
template void tt<AS1 int*>(AS1 int*);
void mo(__attribute__((opencl_global)) int*, __attribute__((opencl_global)) int*, __attribute__((address_space(256))) char*) {}
