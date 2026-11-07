// GNU `noreturn` is a bit of the function type (clang's `ExtInfo`
// `NoReturn`); adding it keeps every other bit of the type as declared:
// the exception specification ([dcl.fct]/1 since C++17, [except.spec]),
// cv- and ref-qualifiers of a member function, the ellipsis, through a
// typedef, `decltype` or a dependent `noexcept(B)` alike. kiln rebuilt the
// type from its canonical form, so a typedef'd type with a dependent
// `noexcept(B)` failed to instantiate. The type with `noreturn` is
// distinct from the one without, and converts to it.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++17
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;
typedef void (*NRT)() noexcept __attribute__((noreturn));
typedef void (*NRF)() __attribute__((noreturn));
typedef void (*NRV)(int, ...) noexcept __attribute__((noreturn));
struct A {
  void m() const & noexcept __attribute__((noreturn));
  void n() volatile && __attribute__((noreturn));
  static void s() noexcept __attribute__((noreturn));
};
typedef void (A::*NRM)() const & noexcept __attribute__((noreturn));
typedef void (A::*NRN)() volatile && __attribute__((noreturn));

static void die() noexcept __attribute__((noreturn));
static void die() noexcept { __builtin_abort(); }
[[gnu::noreturn]] void die2() noexcept(true);
__attribute__((noreturn)) void thr();
void var(int, ...) noexcept __attribute__((noreturn));
__attribute__((noreturn)) auto tr() noexcept -> void;
static_assert(same<decltype(&die), NRT>);
static_assert(!same<decltype(&die), void (*)() noexcept>);
static_assert(same<decltype(&die2), NRT>);
static_assert(same<decltype(&thr), NRF>);
static_assert(same<decltype(&var), NRV>);
static_assert(same<decltype(&tr), NRT>);
static_assert(same<decltype(&A::m), NRM>);
static_assert(same<decltype(&A::n), NRN>);
static_assert(same<decltype(&A::s), NRT>);

// Through a typedef and `decltype`.
typedef void F() noexcept;
F d1 __attribute__((noreturn));
void d0() noexcept;
decltype(d0) d2 __attribute__((noreturn));
static_assert(same<decltype(&d1), NRT>);
static_assert(same<decltype(&d2), NRT>);

// A dependent exception specification, as written and through a typedef.
template <bool B> struct S {
  typedef void G() noexcept(B);
  static G e __attribute__((noreturn));
  static decltype(d1) f;
  static void g() noexcept(B) __attribute__((noreturn));
};
static_assert(same<decltype(&S<true>::e), NRT>);
static_assert(same<decltype(&S<false>::e), NRF>);
static_assert(same<decltype(&S<true>::g), NRT>);
static_assert(same<decltype(&S<false>::g), NRF>);
template <class T> struct U {
  static void e() noexcept(sizeof(T) > 1) __attribute__((noreturn));
};
static_assert(same<decltype(&U<int>::e), NRT>);
static_assert(same<decltype(&U<char>::e), NRF>);

// The pointer converts to one without `noreturn` and without `noexcept`.
void (*p1)() noexcept = &die;
void (*p2)() = &die;
NRT p3 = &die;
void (A::*p4)() const & noexcept = &A::m;
