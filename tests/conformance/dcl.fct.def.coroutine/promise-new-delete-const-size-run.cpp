// [dcl.fct]/5, [dcl.fct.def.coroutine]/9,12: a parameter's top-level
// const is not part of the function type, so a promise's
// `operator new(const std::size_t)` allocates the frame and its
// `operator delete(void *, const std::size_t)` is a usual deallocation
// function that frees it (Boost.Cobalt's `promise_memory_resource_base`;
// kiln reported "no suitable member 'operator delete'", WP-812).
//@ expect: run
//@ std: c++20
//@ stdout: new
//@ stdout: body
//@ stdout: delete sized
#include <coroutine>
#include <cstddef>
#include <cstdlib>
extern "C" int printf(const char*, ...);
struct base {
  static void* operator new(const std::size_t size) {
    printf("new\n");
    return std::malloc(size);
  }
  static void operator delete(void* raw, const std::size_t size) noexcept {
    printf("delete %s\n", size > 0 ? "sized" : "unsized");
    std::free(raw);
  }
};
struct G {
  struct promise_type : base {
    G get_return_object() { return {}; }
    std::suspend_never initial_suspend() { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void unhandled_exception() {}
    void return_void() {}
  };
};
G f() {
  printf("body\n");
  co_return;
}
int main() { f(); }
