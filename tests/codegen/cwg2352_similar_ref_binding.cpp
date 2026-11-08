// [dcl.init.ref]/5.1 with CWG2352: a `const T *const &` bound to a `T *`
// glvalue binds directly (a qualification conversion of the referee, no
// temporary alloca and store): returning a member pointer by
// `const T *const &` returns the member's address, and a local
// reference is the pointer's own address. An array of similar pointers
// binds the same way; a prvalue still materializes a temporary.
struct V { int *b; int *e; };
const int *const &first(const V &v) { return v.b; }
const int *const &last(V &v) { return v.e; }
bool same(int *p) { const int *const &r = p; return &r == (const int *const *)&p; }
const volatile int *const &cv(int *&p) { return p; }
const int *const *const &pp(int **&q) { return q; }
const int *const &xv(int *&p) { return static_cast<int *&&>(p); }
int *arr[2];
int second() { const int *const (&ra)[2] = arr; return *ra[1]; }
int tmp(int *p) { const int *const &r = p + 1; return *r; }
