// The definitions for run-client-check-declared-template*.cpp, compiled
// without any check of their own.
#include "declared-template-contracts.h"
template <class T> int S::m(const int x, T t) const pre(x > 0) post(r: r > x) { return 1; }
template int S::m<int>(int, int) const;
template <class T> int f(const int x, T t) pre(x > 0) post(r: r > x) { return 1; }
template int f<int>(int, int);
template <> int f<long>(const int x, long t) pre(x > 5) { return 2; }
template <class T> int C<T>::m(const int x, T t) pre(x > 0) post(r: r > x) { return 1; }
template struct C<int>;
template int e<int>(int, int);
template <std::size_t I> int P::get() const pre(v > 0) { return int(I); }
template int P::get<0>() const;
template int P::get<1>() const;
