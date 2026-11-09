// [driver] libstdc++ 14's <bits/atomic_wait.h> spin policy, without the
// header: `__thread_relax()` is `__builtin_ia32_pause()` on x86-64, called
// from an inline spin loop that every std::atomic wait, semaphore, latch
// and barrier instantiates (WP-529; the header-level test is
// atomics.wait/run-libstdcxx-wait-notify-x86_64.cpp, x86-64 lane only).
// Pinned to x86-64 so both lanes compare the lowering
// (`call void @llvm.x86.sse2.pause()`) with clang's IR. The clang 19 module. WP-1002.
//@ expect: ir
//@ requires: clang<22
//@ args: --target=x86_64-unknown-linux-gnu
inline void thread_relax() noexcept { __builtin_ia32_pause(); }
template <class Pred> inline bool spin(Pred pred) noexcept {
  for (int i = 0; i < 16; ++i) {
    if (pred())
      return true;
    if (i < 12)
      thread_relax();
  }
  return false;
}
bool wait_nonzero(const volatile int *p) {
  return spin([p] { return *p != 0; });
}
