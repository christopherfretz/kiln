// [expr.await]/5.7, [dcl.init.ref]/5: a reference variable bound to a
// co_await whose `await_resume` returns an lvalue reference designates
// that object, also when the full-expression's cleanups (the awaiter
// temporary) are shared with the suspension's destroy path, and when the
// reference lives across a later suspension (captured by a lambda).
//@ expect: run
//@ stdout: 42 7 43
#include <coroutine>
#include <cstdio>

struct Tok {
  int v;
};

template <class T> struct ready {
  T value_;
  bool await_ready() noexcept { return true; }
  void await_suspend(std::coroutine_handle<>) noexcept {}
  T await_resume() noexcept { return static_cast<T&&>(value_); }
};

struct Counted {
  Tok* p;
  ~Counted() {}
  bool await_ready() noexcept { return false; }
  void await_suspend(std::coroutine_handle<>) noexcept {}
  const Tok& await_resume() noexcept { return *p; }
};

struct tok_t {};

struct Task {
  struct promise_type {
    Tok tok{42};
    Task get_return_object() {
      return {std::coroutine_handle<promise_type>::from_promise(*this)};
    }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
    auto await_transform(tok_t) noexcept { return ready<const Tok&>{tok}; }
    template <class A> A&& await_transform(A&& a) noexcept { return static_cast<A&&>(a); }
  };
  std::coroutine_handle<promise_type> h;
};

std::coroutine_handle<> saved;

struct Sus {
  bool await_ready() noexcept { return false; }
  void await_suspend(std::coroutine_handle<> h) noexcept { saved = h; }
  void await_resume() noexcept {}
};

Tok seven{7};

Task f(int n) {
  while (n > 0) {
    if (n == 1) {
      {
        const auto& ct = co_await tok_t{};
        auto l = [&] { return ct.v; };
        co_await Sus{};
        const Tok& s = co_await Counted{&seven};
        std::printf("%d %d", l(), s.v);
        ++const_cast<Tok&>(ct).v;
      }
      co_return;
    }
    --n;
  }
}

int main() {
  Task t = f(3);
  saved.resume();
  t.h.resume();
  std::printf(" %d\n", t.h.promise().tok.v);
  t.h.destroy();
}
