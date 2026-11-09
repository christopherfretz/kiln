// [over.load]/2.3 valid twin: conversion functions overloaded on two
// ref-qualifiers, member templates, and a dependent conversion in a class
// template that is never instantiated are well-formed (WP-1041).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
struct A {
  operator int() &;
  operator int() &&;
  A &operator=(const A &) &;
  A &operator=(const A &) &&;
  template<class U> operator U*() &;
  template<class U> operator U*();
};
template<class X> struct C {
  operator X() &;
  operator X() const;
};
