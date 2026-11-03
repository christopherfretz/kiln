// [basic.start.dynamic]/1 orders dynamic initialization only within a
// translation unit; GNU `init_priority(N)` (spec/sema.md, spec/codegen.md
// §7f) runs a namespace-scope object's initializer from the
// `_GLOBAL__I_<N>` constructor of priority N, lower first, in lexical
// order within one priority. Unordered (template) initializers keep their
// own entries; `thread_local` objects ignore the priority.
int side(int);
struct A { A(int); ~A(); };
A a __attribute__((init_priority(200))) = A(1);
A b = A(2);
A c __attribute__((init_priority(101))) = A(3);
int i = side(4);
A d __attribute__((init_priority(200))) = A(5);
extern A e;
A e __attribute__((init_priority(65535))) = A(6);
[[gnu::init_priority(300)]] A f = A(7);
template <class T> struct W { static A m; };
template <class T> A W<T>::m = A(8);
inline A il = A(9);
thread_local A t __attribute__((init_priority(150))) = A(10);
A *use() { (void)&W<int>::m; return &t; }
