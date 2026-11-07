// GNU `noreturn` is part of the function type (clang's `ExtInfo`
// `NoReturn`), on a friend declaration as on any other: the friend's own
// type is `noreturn`, so is a later redeclaration's (the attribute is
// inherited; [dcl.attr.noreturn]/1 for `[[noreturn]]`), and so is that
// of a hidden friend found by argument-dependent lookup. kiln dropped the
// attribute from a friend declaration's type, so `&f` lost the bit and a
// function ending in a call did not count as ending (-Wreturn-type).
//@ expect: compile-ok
//@ strict-diags
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;
typedef void (*NRF)() __attribute__((noreturn));
typedef void (*NRT)() noexcept __attribute__((noreturn));
typedef void (*NRI)(int) __attribute__((noreturn));
struct C {
  friend void d1() __attribute__((noreturn));
  friend void d2() noexcept __attribute__((nothrow, noreturn));
  friend __attribute__((noreturn)) void d3();
  friend void h(C) __attribute__((noreturn)) {
    throw 1;
  }
};
void d1();
void d2() noexcept;
void d3();
static_assert(same<decltype(&d1), NRF>);
static_assert(same<decltype(&d2), NRT>);
static_assert(same<decltype(&d3), NRF>);
int r1() { d1(); }
int r2() { d2(); }
int r3(C c) { h(c); }
template <class T> struct D {
  friend void e(D) __attribute__((noreturn)) { throw 1; }
};
int r4(D<int> x) { e(x); }
void d1() { throw 1; }
