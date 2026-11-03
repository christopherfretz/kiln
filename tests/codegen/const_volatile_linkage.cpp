// [basic.link]/3.2: only a variable of non-volatile const-qualified type
// gets internal linkage from its type, so a namespace-scope
// `const volatile` variable (or array of them, [basic.type.qualifier]/3)
// has external linkage and an unmangled external symbol; `static` or an
// unnamed namespace still makes it internal, and a const pointer to
// volatile stays internal.
const volatile int cv = 1;
static const volatile int scv = 2;
extern const volatile int ecv;
const volatile int ecv = 3;
const volatile int arr[2] = {4, 5};
const volatile int ar2[2][2] = {};
struct A { int x; };
const volatile A ca = {1};
const int ci = 6;
volatile const int *const p = nullptr;
namespace { const volatile int anon = 7; }
namespace n { const volatile int ncv = 8; }
const volatile int *f() { return &cv; }
const volatile int *g() { return &scv; }
const volatile int *h() { return &ecv; }
const volatile int *k() { return &arr[1]; }
const volatile int *l() { return &ar2[1][1]; }
const int *m() { return &ci; }
const volatile int *o() { return &anon; }
const volatile int *q() { return &n::ncv; }
int r() { return ca.x; }
volatile const int *const *s() { return &p; }
