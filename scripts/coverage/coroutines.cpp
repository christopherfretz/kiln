// Coverage comparison program (scripts/coverage-compare.py): a generator
// coroutine.
#include <coroutine>
#include <cstdio>
struct Gen {
  struct promise_type {
    int cur = 0;
    Gen get_return_object() { return Gen{std::coroutine_handle<promise_type>::from_promise(*this)}; }
    std::suspend_always initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    std::suspend_always yield_value(int v) { cur = v; return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
  std::coroutine_handle<promise_type> h;
  ~Gen() { if (h) h.destroy(); }
  bool next() { h.resume(); return !h.done(); }
};
Gen count(int n) {
  for (int i = 0; i < n; ++i) {
    if (i % 2) co_yield i * 10;
    else co_yield i;
  }
}
int main() {
  Gen g = count(5);
  int s = 0;
  while (g.next()) s += g.h.promise().cur;
  std::printf("%d\n", s);
}
