// [temp.point]/1, [expr.prim.id.general]/4: an implicit instantiation runs
// in the context of its template, not inside the full-expression that
// required it. A non-static data member named without an object (here
// `&(S::m)`, which [expr.unary.op]/4 makes no pointer to member, and the
// call argument `S::n`) is an error of the full-expression that names it;
// completing an associated class for argument-dependent lookup of the
// unary `&` or the call (`Assoc<...>`, whose `static_assert` is a
// full-expression of its own) neither reports it from inside that
// instantiation nor consumes it. The `std::pair<const int, int>` reached
// through `std::map<int, int>`'s allocator argument has the same shape.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "invalid use of non-static data member 'm'"
//@ diag: 21:14 error "invalid use of non-static data member 'n'"
template <int N> struct Tag { static constexpr int id = N; };
template <class T> struct Assoc { static_assert(T::id > 0); };
template <class T, class A = Assoc<T>> struct Box {};
struct S { Box<Tag<1>> m; Box<Tag<2>> n; };
template <class T> int g(T) { return 0; }
auto p = &(S::m);
int x = g(S::n);
