// Failed new-, delete- and await-expressions (spec/ast.md §18.13.5,
// stage 6c): a new-expression with no matching allocation function
// ([expr.new]/12), a delete-expression of a class whose deallocation
// function is inaccessible ([expr.delete]/12), and an await-expression
// whose `operator co_await` is ambiguous ([over.match.oper]/3) are
// diagnosed and have an `UNRESOLVED` entry at their keyword naming
// nothing (`::` as the lead of `::new`). One whose operand is erroneous
// has none.
//@ expect: compile-fail
//@ std: c++20
//@ written-refs: operators-new-delete-errors.refs
//@ diag: 58:3 error "no matching function for call to 'operator new'"
//@ diag: 59:3 error "no matching 'operator new' function for non-allocating placement new expression; include <new>"
//@ diag: 60:3 error "'operator delete' is a private member of 'Hidden'"
//@ diag: 61:10 error "use of undeclared identifier 'undeclared_ptr'"
//@ diag: 64:3 error "use of overloaded operator 'co_await' is ambiguous (operand type 'Two')"
//@ diag: 65:12 error "use of undeclared identifier 'undeclared_ptr'"
using size_t = decltype(sizeof 0);
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
struct task {
  struct promise_type {
    task get_return_object() { return {}; }
    Ready initial_suspend() { return {}; }
    Ready final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};

struct NoPlace {
  static void *operator new(size_t);
};
class Hidden {
  void operator delete(void *);
};
struct Two {};
Ready operator co_await(Two);
Ready operator co_await(const Two &);

void use(void *b, Hidden *h) {
  new (b) NoPlace;
  ::new (b) int;
  delete h;
  delete undeclared_ptr;
}
task coro() {
  co_await Two{};
  co_await undeclared_ptr;
}
