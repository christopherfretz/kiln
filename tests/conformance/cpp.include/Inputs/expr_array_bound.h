#ifndef EXPR_ARRAY_BOUND_H
#define EXPR_ARRAY_BOUND_H
template <class T> struct ab_buf { T data[sizeof(T) + 1]; };
template <int N> struct ab_twice { char b[N * 2]; };
template <int N> int ab_count(const int (&)[N + 1]) { return N; }
template <class T> using ab_arr = T[sizeof(T)];
#endif
