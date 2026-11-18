// GCC's __is_invocable(F, Args...) and __is_nothrow_invocable(F, Args...):
// INVOKE(declval<F>(), declval<Args>()...) ([func.require]/1) is
// well-formed (and does not throw). __has_builtin answers 0 for both, as
// clang 22 does, so libstdc++ keeps its library traits, and both names stay
// identifiers unless followed by '('. Found running GCC's C++ testsuite.
// WP-1667.
//@ std: c++20
//@ expect: compile-ok
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __is_invocable
//@ strict-diags
#include <functional>

#if defined(__KILN_MAJOR__) && (__has_builtin(__is_invocable) || __has_builtin(__is_nothrow_invocable))
#error "kiln answers 0, so libstdc++ keeps its own std::is_invocable"
#endif

// The names as libstdc++ 14 uses them: class templates.
template <class F, class... A> struct __is_invocable { static const bool value = false; };
static_assert(!__is_invocable<int>::value);

struct Base {
  int data;
  int get() &;
  int cget() const;
  int rget() &&;
  int nx(long) noexcept;
};
struct Derived : Base {};
struct Callable {
  int operator()(int) noexcept;
  void operator()(int, int);
private:
  void operator()(long *);
};
struct Ptr { Base &operator*(); };

static_assert(__is_invocable(void (*)()));
static_assert(!__is_invocable(void (*)(), int));
static_assert(__is_invocable(int (&)(int), short));
static_assert(!__is_invocable(int (*)(int &), int));
static_assert(__is_invocable(int (*)(int &&), int));
static_assert(!__is_invocable(int));
static_assert(!__is_invocable(void (*)(), void));

static_assert(__is_invocable(int Base::*, Base &));
static_assert(__is_invocable(int Base::*, const Derived *));
static_assert(__is_invocable(int Base::*, Ptr));
static_assert(!__is_invocable(int Base::*, Base &, int));
static_assert(!__is_invocable(int Base::*, int *));
static_assert(!__is_invocable(int Base::*));

static_assert(__is_invocable(int (Base::*)() &, Base &));
static_assert(!__is_invocable(int (Base::*)() &, Base));
static_assert(__is_invocable(int (Base::*)() &&, Derived));
static_assert(!__is_invocable(int (Base::*)() &, const Base &));
static_assert(__is_invocable(int (Base::*)() const, const Base *));
static_assert(__is_invocable(int (Base::*)(long), Derived &, int));
static_assert(!__is_invocable(int (Base::*)(long), Derived &, int *));

static_assert(__is_invocable(int (Base::*)() &, std::reference_wrapper<Derived>));
static_assert(!__is_invocable(int (Base::*)() &&, std::reference_wrapper<Base>));
static_assert(__is_invocable(int Base::*, std::reference_wrapper<const Base>));
static_assert(__is_invocable(std::reference_wrapper<int (int)>, int));

static_assert(__is_invocable(Callable, int));
static_assert(__is_invocable(Callable &, int, int));
static_assert(!__is_invocable(Callable, long *));
static_assert(!__is_invocable(const Callable, int));

static_assert(__is_nothrow_invocable(Callable, int));
static_assert(!__is_nothrow_invocable(Callable, int, int));
static_assert(__is_nothrow_invocable(int (Base::*)(long) noexcept, Base *, long));
static_assert(!__is_nothrow_invocable(int (Base::*)() const, Base *));
static_assert(__is_nothrow_invocable(int Base::*, Base &));
static_assert(!__is_nothrow_invocable(Callable, long *));

template <class F, class... A>
constexpr bool invocable_v = __is_invocable(F, A...);
static_assert(invocable_v<Callable, int>);
static_assert(!invocable_v<Callable, Base>);
static_assert(invocable_v<decltype([](auto x) { return x; }), int>);

struct Incomplete;
static_assert(__is_invocable(int (*)(Incomplete &), Incomplete &));
static_assert(__is_invocable(int Incomplete::*, const Incomplete &));
