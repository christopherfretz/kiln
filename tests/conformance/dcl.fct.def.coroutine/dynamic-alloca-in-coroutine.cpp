// [dcl.fct.def.coroutine]: a coroutine's frame has a fixed size, so its
// body cannot allocate on the stack dynamically. kiln rejects, once per
// coroutine at the first, a call of `__builtin_alloca`, `alloca`,
// `__builtin_alloca_with_align` or an `_uninitialized` form, and a
// variable length array that an instantiation forms. clang accepts both
// and then fails in its back end ("Coroutines cannot handle non static
// allocas yet"): kiln is stricter (docs/status/deviations.md). An
// unevaluated call, a nested lambda and an ordinary function are fine.
// GNU mode, where `alloca` is a library builtin lowered inline (in strict
// mode it is an ordinary call, as in clang). WP-1374.
//@ expect: compile-fail
//@ args: -std=gnu++20 -Wno-vla-cxx-extension
//@ skip(clang): clang accepts these and crashes in its back end
//@ strict-diags
//@ diag: 37:28 error "a call to '__builtin_alloca' in a coroutine is not supported: the coroutine frame cannot hold a dynamically sized stack allocation"
//@ diag: 38:54 error "a call to 'alloca' in a coroutine is not supported: the coroutine frame cannot hold a dynamically sized stack allocation"
//@ diag: 39:22 error "a call to '__builtin_alloca_with_align' in a coroutine is not supported: the coroutine frame cannot hold a dynamically sized stack allocation"
//@ diag: 40:22 error "a call to '__builtin_alloca_uninitialized' in a coroutine is not supported: the coroutine frame cannot hold a dynamically sized stack allocation"
//@ diag: 41:71 error "a call to '__builtin_alloca' in a coroutine is not supported: the coroutine frame cannot hold a dynamically sized stack allocation"
//@ diag: 42:15 note "in instantiation of function template specialization 'a5<int>' requested here"
//@ diag: 47:40 error "a variable length array instantiated from a template in a coroutine is not supported: the coroutine frame cannot hold a dynamically sized stack allocation"
//@ diag: 48:15 note "in instantiation of function template specialization 'v1<int>' requested here"
#include "Inputs/coro.h"
extern "C" void* alloca(decltype(sizeof 0));
void use(void*);

struct agen {
  struct promise_type {
    agen get_return_object() { return {}; }
    std::suspend_always initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};

agen a1(int n) { void* p = __builtin_alloca(n); use(p); co_return; }
agen a2(int n) { co_await std::suspend_always{}; use(alloca(n)); use(alloca(n)); }
agen a3(int n) { use(__builtin_alloca_with_align(n, 64)); co_return; }
agen a4(int n) { use(__builtin_alloca_uninitialized(n)); co_return; }
template <class T> agen a5(T n) { co_await std::suspend_always{}; use(__builtin_alloca(n)); }
template agen a5<int>(int);
agen ok1(int n) { use((void*)sizeof(__builtin_alloca(n))); co_return; }
agen ok2(int n) { auto l = [n] { use(__builtin_alloca(n)); }; l(); co_return; }
void ok3(int n) { use(__builtin_alloca(n)); }

template <class T> agen v1(T n) { int a[n]; use(a); co_return; }
template agen v1<int>(int);
