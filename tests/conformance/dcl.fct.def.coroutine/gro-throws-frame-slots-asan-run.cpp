// [dcl.fct.def.coroutine]/5, /11: when `get_return_object()` throws, the
// ramp frees the frame and the exception propagates to the caller. With a
// handle-returning final `await_suspend`, LLVM's CoroSplit keeps the
// ramp's exception-object and selector slots in the frame, and the ramp
// reads them after freeing it to resume unwinding: a use after free, here
// an AddressSanitizer report. clang 19
// and 22 emit the same IR and fail the same way, so kiln does not work
// around it (docs/KNOWN_ISSUES.md "LLVM code generator bugs"; found by
// the hod project, github.com/christopherfretz/hod). WP-1374.
//@ expect: run
//@ std: c++20
//@ args: -fsanitize=address
//@ requires: sanitizer-runtime
//@ env: ASAN_OPTIONS=detect_leaks=0
//@ stdout: caught 1
//@ xfail: LLVM CoroSplit keeps the ramp's exception slots in the frame it frees before reading them (clang 19 and 22 alike)
#include "Inputs/coro.h"
extern "C" int printf(const char*, ...);

bool fail = false;

struct gtask {
  struct promise_type {
    gtask get_return_object() {
      if (fail) throw 1;
      return {};
    }
    std::suspend_always initial_suspend() noexcept { return {}; }
    // A symmetric transfer at the final suspend: its landing pads are
    // what makes LLVM put the slots in the frame.
    struct fin {
      bool await_ready() noexcept { return false; }
      std::coroutine_handle<> await_suspend(std::coroutine_handle<>) noexcept {
        return std::noop_coroutine();
      }
      void await_resume() noexcept {}
    };
    fin final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() { throw; }
  };
};

gtask leaf(int i) {
  if (i == 3) throw 2;
  co_return;
}

int main() {
  fail = true;
  try {
    leaf(0);
  } catch (int e) {
    printf("caught %d\n", e);
  }
}
