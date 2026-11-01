// libc++ <type_traits> edge cases (WP-176):
// - __decay of an abominable template argument is the type itself (clang's
//   BuiltinDecay delegates to __add_pointer, which cannot form the
//   pointer); an ordinary function type decays to its pointer.
// - [dcl.init.ref]/5.3.1: a function lvalue binds to an rvalue reference
//   to function, so is_convertible<F, F&&> and <F&, F&&> hold.
// - [meta.rel]/5: conversion to an abstract class type is never possible
//   (the imagined `To test()` is ill-formed), even from To itself.
//@ expect: compile-ok
//@ strict-diags
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;
static_assert(same<__decay(int(int)), int (*)(int)>);
using AR = int(int) &&;
using AC = void() const;
template <class T> using decay_t = __decay(T);
static_assert(same<decay_t<AR>, AR>);
static_assert(same<decay_t<AC>, AC>);
static_assert(same<__decay(int[3]), int*>);

int f(int);
using F = int(int);
static_assert(__is_convertible(F, F&&));
static_assert(__is_convertible(F&, F&&));
static_assert(__is_convertible(F&&, F&));
F&& rf = f;
int (&&rf2)(int) = static_cast<F&>(f);

struct Abs { virtual void g() = 0; };
struct D : Abs { void g() override; };
static_assert(!__is_convertible(Abs, Abs));
static_assert(!__is_convertible(D, Abs));
static_assert(__is_convertible(D&, Abs&));
static_assert(__is_convertible(D*, Abs*));
