//@ args: -O1
// [basic.stc.thread], [stmt.dcl]/3: at -O1 `__tls_init` marks `__tls_guard`
// invariant once set (clang `GenerateCXXGlobalInitFunc` →
// `EmitInvariantStart`: `llvm.invariant.start.p0(i64 1, ptr @__tls_guard)`),
// which keeps the optimizer from shrinking the guard to an `i1`. A
// template's thread-local variable has its own guarded `_ZTH` function
// and no `__tls_guard`.
int f();
thread_local int t = f();
template <class T> struct X { static thread_local T v; };
template <class T> thread_local T X<T>::v = T(f());
int g() { return t + X<long>::v; }
