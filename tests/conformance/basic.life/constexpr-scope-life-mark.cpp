// [basic.life]/9, [expr.const]/10.8 (WP-760): leaving a scope checks the
// objects it destroys whose lifetime an explicit destructor call may have
// ended (spec/oven.md "Object lifetime"); a scope above every such object
// of the evaluation skips the check. Objects tracked in an enclosing
// scope, in an ended scope or in a caller do not hide one tracked later:
// its destruction at its scope's end is still diagnosed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:7 note "destroying object 't' whose lifetime has already ended"
//@ diag: 60:15 note "in call to 'inner_after_outer(true)'"
//@ diag: 77:15 error "static assertion expression is not an integral constant expression"
//@ diag: 70:7 note "destroying object 'u' whose lifetime has already ended"
//@ diag: 77:15 note "in call to 'after_release(true)'"
//@ diag: 93:15 error "static assertion expression is not an integral constant expression"
//@ diag: 81:5 note "destroying object 't' whose lifetime has already ended"
//@ diag: 90:10 note "in call to 'callee(true)'"
//@ diag: 93:15 note "in call to 'in_callee(true)'"
namespace std {
using size_t = decltype(sizeof 0);
}
constexpr void* operator new(std::size_t, void* p) noexcept { return p; }
namespace std {
template <class T, class... A> constexpr T* construct_at(T* p, A&&... a) { return ::new ((void*)p) T(static_cast<A&&>(a)...); }
}

struct T { int a, b; };

// Tracked in the enclosing scope: the inner scopes destroy nothing
// tracked, and the object itself is fine at its own scope's end.
constexpr int outer_tracked() {
  T keep{1, 2};
  keep.~T();
  std::construct_at(&keep, 3, 4);
  int r = 0;
  for (int i = 0; i < 4; ++i) {
    T t{i, i};
    r += t.a;
  }
  return r + keep.a;
}
static_assert(outer_tracked() == 9);

// Tracked below the scope, then one tracked in it: still checked.
constexpr bool inner_after_outer(bool end) {
  T keep{1, 2};
  keep.~T();
  std::construct_at(&keep, 3, 4);
  for (int i = 0; i < 2; ++i) {
    T t{i, i};
  }
  {
    T t{1, 2};
    if (end)
      t.~T();
  }
  return true;
}
static_assert(inner_after_outer(false));
static_assert(inner_after_outer(true));

// Tracked in a scope that has ended, then a later scope.
constexpr bool after_release(bool end) {
  {
    T t{1, 2};
    t.~T();
    std::construct_at(&t, 1, 2);
  }
  {
    T u{1, 2};
    if (end)
      u.~T();
  }
  return true;
}
static_assert(after_release(false));
static_assert(after_release(true));

// Tracked in a callee's frame, above the caller's scopes.
constexpr bool callee(bool end) {
  T t{1, 2};
  if (end)
    t.~T();
  return true;
}
constexpr bool in_callee(bool end) {
  T keep{1, 2};
  keep.~T();
  std::construct_at(&keep, 3, 4);
  return callee(end);
}
static_assert(in_callee(false));
static_assert(in_callee(true));
