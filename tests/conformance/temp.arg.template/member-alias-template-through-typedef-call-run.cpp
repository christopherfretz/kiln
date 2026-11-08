// [temp.arg.template]/1, [temp.names]/3: `Q::fn`, with `Q` a typedef of the
// specialization `quote<int>`, names the member alias template `fn` and is
// a valid template template argument; `check<Q::fn>(0)` calls the
// specialization. Boost.Mp11 `mp_valid<QX::fn>` (mp_valid.cpp:49) hits
// this.
//@ expect: run
//@ stdout: 7
extern "C" int printf(const char*, ...);
template<class F> struct quote { template<class... T> using fn = int; };
using Q = quote<int>;
template<template<class...> class G> int check(int) { return 7; }

int main() { printf("%d\n", check<Q::fn>(0)); }
