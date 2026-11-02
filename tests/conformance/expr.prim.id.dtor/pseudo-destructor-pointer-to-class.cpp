// [expr.prim.id.dtor]/2, [expr.ref]/2: `p.~T()` with `T` a pointer type is
// a pseudo-destructor call on a scalar object even when the pointee is a
// class (libc++'s `__cpp17_destructible` checks `__v.~_Tp()` for iterator
// types such as `S*`); only naming the pointee asks for `->`.
//@ strict-diags
struct S { int x; };
template <class T> concept Destructible = requires(T v) { v.~T(); };
static_assert(Destructible<S *> && Destructible<const S *> && Destructible<S>);
static_assert(Destructible<int *> && Destructible<int>);
template <class T> void kill(T v) { v.~T(); }
template void kill<S *>(S *);
using P = S *;
void f(P p) { p.~P(); }
