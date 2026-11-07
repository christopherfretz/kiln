// WP-782: __builtin_thread_pointer() lowers to `llvm.thread.pointer`
// (clang CGBuiltin BI__builtin_thread_pointer); the intrinsic is
// address-space overloaded (`llvm.thread.pointer.p0`) from LLVM 20.

void *f() { return __builtin_thread_pointer(); }
long g(long off) { return (long)__builtin_thread_pointer() + off; }
