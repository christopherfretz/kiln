// [dcl.fct.def.coroutine]/9: the promise's `operator new` templates take
// part in the lookup with the parameters appended, as in a
// new-expression: `template <class A> operator new(size_t,
// allocator_arg_t, A)` allocates the frame of `g(allocator_arg, a)`, and
// a pack template takes `(int, long&, char&)`; the plain form is the
// fallback.
//@ expect: run
//@ std: c++20
//@ stdout: 1 2 2
#include <coroutine>
#include <cstdio>
#include <cstdlib>
#include <memory>
static int news = 0, plain = 0, sized = 0;
struct Task {
  struct promise_type {
    template <class A>
    static void *operator new(std::size_t n, std::allocator_arg_t, A) { ++news; return std::malloc(n); }
    template <class... Ts>
    static void *operator new(std::size_t n, int, Ts &...) { ++sized; return std::malloc(n); }
    static void *operator new(std::size_t n) { ++plain; return std::malloc(n); }
    static void operator delete(void *p, std::size_t) { std::free(p); }
    Task get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};
struct Al {};
template <class A> Task g(std::allocator_arg_t, A) { co_return; }
Task k(double) { co_return; }
Task m() { co_return; }
Task v(int, long, char) { co_return; }
struct S { Task mem(std::allocator_arg_t, Al) { co_return; } };
int main() {
  g(std::allocator_arg, Al{});
  k(1.0);
  m();
  v(1, 2, 'c');
  S{}.mem(std::allocator_arg, Al{});
  std::printf("%d %d %d\n", news, plain, sized);
}
