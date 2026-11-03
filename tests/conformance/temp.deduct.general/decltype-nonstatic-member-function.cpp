// [temp.deduct.general]/8: `decltype(C<T>::f)` naming a non-static member
// function without an object ([expr.prim.id.general]/4) is invalid in the
// immediate context, so a partial specialization or function template
// whose substitution forms it is not viable. libc++'s `tuple_size<const T>`
// is constrained this way ([dcl.struct.bind]/4 with an empty pack).
//@ expect: compile-ok
//@ strict-diags
template <class T> struct TS;
template <class T, class...> using Imp = T;
template <class T> struct TS<Imp<const T, decltype(TS<T>::value)>> {
  static constexpr int v = 1;
};
struct A {};
template <> struct TS<A> { void value(); };
struct B {};
template <> struct TS<B> { static const int value = 1; };
template <class T> concept Complete = requires { sizeof(T); };
static_assert(!Complete<TS<const A>>);
static_assert(TS<const B>::v == 1);
template <class T> constexpr bool f(decltype(TS<T>::value)*) { return true; }
template <class T> constexpr bool f(...) { return false; }
static_assert(!f<A>(nullptr) && f<B>(nullptr));
