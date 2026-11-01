// [coroutine.handle], clang `CoroLangBuiltin`s: the `__builtin_coro_*`
// functions the libc++ headers name are declared with clang 19's types
// (`done`/`promise`/`noop`/... are `noexcept`, `resume`/`destroy` are
// not). An `I` argument (`promise`'s alignment and direction, `id`'s
// alignment, `end`/`suspend`'s flag) must be an integer constant
// expression, checked at instantiation when value-dependent. Constant
// evaluation rejects every coroutine builtin.
//@ expect: compile-fail
//@ diag: 18:9 error "argument to '__builtin_coro_promise' must be a constant integer"
//@ diag: 19:11 error "argument to '__builtin_coro_end' must be a constant integer"
//@ diag: 21:29 error "cannot initialize a parameter of type 'void *' with an rvalue of type 'int'"
//@ diag: 28:9 error "argument to '__builtin_coro_id' must be a constant integer"
//@ diag: 35:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34:37 note "subexpression not valid in a constant expression"
//@ diag: 35:15 note "in call to 'cd(nullptr)'"
template <class T> void *pr(void *p) { return __builtin_coro_promise(p, alignof(T), false); }
void g(void *p, int n, bool b) {
  (void)__builtin_coro_promise(p, n, false);
  (void)  __builtin_coro_end(p, b);
  (void)__builtin_coro_suspend(1 + 1);
  (void)__builtin_coro_done(1);
  (void)pr<long>(p);
  static_assert(noexcept(__builtin_coro_done(p)));
  static_assert(noexcept(__builtin_coro_promise(p, 8, true)));
  static_assert(noexcept(__builtin_coro_noop()));
  static_assert(!noexcept(__builtin_coro_resume(p)));
  static_assert(!noexcept(__builtin_coro_destroy(p)));
  (void)__builtin_coro_id(n, p, p, p);
  bool d = __builtin_coro_done(p);
  void *q = __builtin_coro_noop();
  __SIZE_TYPE__ s = __builtin_coro_size() + __builtin_coro_align();
  (void)d, (void)q, (void)s;
}
constexpr bool cd(void *p) { return __builtin_coro_done(p); }
static_assert(cd(nullptr));
