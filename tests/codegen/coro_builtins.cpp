// `__builtin_coro_*` outside a coroutine body (clang
// `EmitCoroutineIntrinsic`, CodeGenCoroutines/coro-builtins.c): each maps
// 1:1 to its `llvm.coro.*` intrinsic; a hand-written `__builtin_coro_id`
// supplies the token later `coro.alloc`/`begin`/`free` calls take and
// `__builtin_coro_begin` the handle `__builtin_coro_frame` returns
// (`createCoroData`); `coro.suspend` gets a `none` save token. No
// `presplitcoroutine`: the function is not a C++ coroutine
// ([dcl.fct.def.coroutine]/1); spec/codegen.md §7p. `__builtin_coro_end`
// (whose `llvm.coro.end` type follows the LLVM release) is
// tests/lower.rs `coro_end_follows_llvm_major`.
void *myAlloc(long long);
void sink(void *);
void f() {
  __builtin_coro_id(32, nullptr, nullptr, nullptr);
  bool a = __builtin_coro_alloc();
  sink(__builtin_coro_noop());
  __builtin_coro_begin(a ? myAlloc(__builtin_coro_size()) : nullptr);
  __builtin_coro_resume(__builtin_coro_frame());
  __builtin_coro_destroy(__builtin_coro_frame());
  bool d = __builtin_coro_done(__builtin_coro_frame());
  sink(__builtin_coro_promise(__builtin_coro_frame(), 48, false));
  sink(__builtin_coro_free(__builtin_coro_frame()));
  sink(&d);
  char s = __builtin_coro_suspend(true);
  (void)s;
  (void)__builtin_coro_align();
}
