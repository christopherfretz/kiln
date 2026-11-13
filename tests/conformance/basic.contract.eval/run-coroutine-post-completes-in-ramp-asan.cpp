// [dcl.contract.func], [dcl.fct.def.coroutine]/7 (P2900R14 §3.5.2): a
// coroutine's postcondition is checked by the ramp after the coroutine
// has run to completion and freed its frame (both suspends are
// `suspend_never`), on the return object. The return object must not
// live in that frame: kiln keeps the return slot outside it
// (`!coro.outside.frame`) when a postcondition names the result. The
// read of freed memory is an AddressSanitizer report. The plain sibling
// runs it with glibc malloc perturbation.
// Reported by the hod project (github.com/christopherfretz/hod). WP-1374.
//@ expect: run
//@ args: -fcontract-evaluation-semantic=enforce -fsanitize=address
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ requires: sanitizer-runtime
//@ env: ASAN_OPTIONS=detect_leaks=0
//@ stdout: 42
//@ stdout: 43
#include "../dcl.fct.def.coroutine/Inputs/coro.h"
extern "C" int printf(const char*, ...);

struct sync {
  struct promise_type {
    long v;
    promise_type(long x = 42) : v(x) {}
    sync get_return_object() { return {v}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
  long v;
  bool ok() const { return v == 42 || v == 43; }
};

sync f() post(r : r.ok()) { co_return; }
sync g(long x) post(r : r.ok()) { co_return; }

int main() {
  sync s = f();
  printf("%ld\n", s.v);
  sync t = g(43);
  printf("%ld\n", t.v);
}
