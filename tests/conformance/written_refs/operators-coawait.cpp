// `co_await` in the written-reference table (spec/ast.md §18.13.6,
// stage 6c): the keyword names the `operator co_await` applied to the
// operand ([expr.await]/3.3: a member, or a non-member found by
// unqualified or argument-dependent lookup), or to the result of the
// promise's `await_transform` ([expr.await]/3.2). When none applies (the
// awaitable is the awaiter) the keyword has no entry. The calls of
// `await_transform`, `await_ready`, `await_suspend` and `await_resume`,
// `co_yield`'s `yield_value` ([expr.yield]/1) and the initial and final
// suspend points ([dcl.fct.def.coroutine]/5) have no token of their own:
// no entry. In a coroutine template the keyword is a DEPENDENT entry whose
// found set is the definition's unqualified lookup of `operator co_await`
// (flag ADL), with a row per specialization: the `operator co_await`
// applied, or `(builtin)` when none applies; a discarded `if constexpr`
// branch ([stmt.if]/2) has none.
//@ std: c++20
//@ written-refs: operators-coawait.refs
namespace std {
template <class R, class... A> struct coroutine_traits {
  using promise_type = typename R::promise_type;
};
template <class P = void> struct coroutine_handle;
template <> struct coroutine_handle<void> {
  static coroutine_handle from_address(void *) noexcept { return {}; }
  void *address() const noexcept { return nullptr; }
};
template <class P> struct coroutine_handle : coroutine_handle<> {
  static coroutine_handle from_address(void *) noexcept { return {}; }
};
} // namespace std

struct Ready {
  bool await_ready() const noexcept { return true; }
  void await_suspend(std::coroutine_handle<>) const noexcept {}
  void await_resume() const noexcept {}
};
struct Plain {
  bool await_ready() const noexcept { return true; }
  void await_suspend(std::coroutine_handle<>) const noexcept {}
  int await_resume() const noexcept { return 1; }
};
struct Mem {
  Ready operator co_await() const { return {}; }
};
struct Glob {};
Ready operator co_await(Glob);
namespace ns {
struct Free {};
Ready operator co_await(Free);
} // namespace ns

struct task {
  struct promise_type {
    task get_return_object() { return {}; }
    Ready initial_suspend() { return {}; }
    Ready final_suspend() noexcept { return {}; }
    Ready yield_value(int) { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};
struct ttask {
  struct promise_type {
    ttask get_return_object() { return {}; }
    Ready initial_suspend() { return {}; }
    Ready final_suspend() noexcept { return {}; }
    Mem await_transform(int) { return {}; }
    Plain await_transform(char) { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};

task direct() {
  co_await Plain{};
  co_await Mem{};
  co_await Glob{};
  co_await ns::Free{};
  co_yield 1;
}
ttask transformed() {
  co_await 1;
  co_await 'c';
}

template <class T> task await(T t) { co_await t; }
template <class T> task never(T t) { co_await t; }
template <class T> task discard(T t) {
  if constexpr (sizeof(T) > 1000)
    co_await t;
  co_return;
}

void use() {
  await(Mem{});
  await(Plain{});
  await(Glob{});
  await(ns::Free{});
  discard(Mem{});
}
