// [stmt.dcl]/3: a block-scope static whose initializer is not constant
// is initialized the first time control passes through its declaration,
// under a _ZGV guard (__cxa_guard_acquire/release, __cxa_guard_abort if
// the initialization throws); its destructor is registered with
// __cxa_atexit (libc++ support/count_new.h's getGlobalMemCounter).
struct M { struct Arg {}; M(Arg); ~M(); int v; };
struct N { N(); int v; };
int f();
inline M *get() { static M c{M::Arg()}; return &c; }
M &gm = *get();
int h() { static N n; static int k = f(); static const int &r = f(); return n.v + k + r; }
int h2() { static M m = M(M::Arg()); static N arr[2]; return m.v + arr[1].v; }
template <class T> int th() { static T t; return t.v; }
int uth() { return th<N>(); }
struct K { int k; };
const K &kk = K{f()};
K &&kr = K{3};
