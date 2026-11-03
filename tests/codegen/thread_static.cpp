// [basic.stc.thread], clang TLS_Static: a GNU `__thread` or C11
// `_Thread_local` variable is constant-initialized and trivially
// destructible (spec/sema.md §4.10). It is a `thread_local` global read
// through @llvm.threadlocal.address, with no _ZTW wrapper or _ZTH
// initializer, even when it is extern or a static data member.
extern __thread int e;
__thread int a = 1;
_Thread_local int b;
struct C { constexpr C(int v) : v(v) {} int v; };
__thread C c(2);
struct S { static __thread int m; };
__thread int S::m = 4;
int f() { static __thread int l = 3; return e + a + b + c.v + l + S::m; }
