// [temp.constr.atomic]/3, [support.srcloc.cons]/2: as
// p3-srcloc-default-arg-function.cpp, for a member template of a class
// template whose requires-clause calls through the class's dependent
// promise type a function with a `source_location` default argument
// (reduced from a coroutine library's `context<R>::await`, WP-1325).
//@ expect: run
#include <source_location>
template <typename A>
concept awaitable = requires(A &&a) { a.await_ready(); };
struct Aw { bool await_ready(); };
struct Promise {
  template <typename T>
  Aw await_transform(T &&t, const std::source_location &loc = std::source_location::current());
};
template <typename R>
struct context {
  using promise_type = typename R::promise_type;
  template <typename T>
    requires requires(promise_type &pro, T &&aw) {
      { pro.await_transform(static_cast<T &&>(aw)) } -> awaitable;
    }
  int await(T &&) { return 1; }
  int await(...) { return 2; }
};
struct Task { using promise_type = Promise; };
struct Op {};
int main() {
  context<Task> c;
  return c.await(Op{}) == 1 ? 0 : 1;
}
