//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 45:31 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 45:31 note "pointer to subobject of '[a, b]' is not a constant expression"
//@ diag: 44:18 note "address of non-static constexpr variable '[a, b]' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 46:31 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 46:31 note "reference to subobject of '[a, b]' is not a constant expression"
//@ diag: 44:18 note "address of non-static constexpr variable '[a, b]' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 48:31 error "constexpr variable 'q' must be initialized by a constant expression"
//@ diag: 48:31 note "pointer to 'd' is not a constant expression"
//@ diag: 47:22 note "declared here"
//@ diag: 49:31 error "constexpr variable 's' must be initialized by a constant expression"
//@ diag: 49:31 note "reference to 'c' is not a constant expression"
//@ diag: 47:19 note "declared here"
//@ diag: 52:24 error "constexpr variable 'lp' must be initialized by a constant expression"
//@ diag: 52:24 note "pointer to subobject of '[e, g]' is not a constant expression"
//@ diag: 51:8 note "declared here"
//@ diag: 54:18 error "constexpr variable 'tp' must be initialized by a constant expression"
//@ diag: 60:22 error "constexpr variable 'hp' must be initialized by a constant expression"
//@ diag: 60:22 note "pointer to subobject of '[a, b]' is not a constant expression"
//@ diag: 57:18 note "address of non-static constexpr variable '[a, b]' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
// [expr.const.init]/2,4 and [dcl.struct.bind] (C++26, P2686R5): a
// structured binding names a member or element of the decomposed object e
// (or, tuple-like, the object of its holding variable), which is
// constexpr-referenceable exactly as e is: an automatic e is not from a
// static or thread_local variable's initializer (it must be representable
// at namespace scope), nor from a lambda body (another function parameter
// scope), nor once its function has returned. Clang 22's diagnostics.
namespace std {
using size_t = decltype(sizeof 0);
template <class T> struct tuple_size;
template <size_t I, class T> struct tuple_element;
template <class T> requires requires { tuple_size<T>::value; }
struct tuple_size<const T> { static constexpr size_t value = tuple_size<T>::value; };
template <size_t I, class T> struct tuple_element<I, const T> { using type = const typename tuple_element<I, T>::type; };
} // namespace std
struct S { int x, y; };
struct X { template <std::size_t I> constexpr int get() const { return int(I) + 10; } };
template <> struct std::tuple_size<X> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, X> { using type = int; };
constexpr S gs{1, 2};
void f() {
  constexpr auto [a, b] = gs;
  static constexpr const int *p = &b;
  static constexpr const int &r = a;
  constexpr auto [c, d] = X{};
  static constexpr const int *q = &d;
  static constexpr const int &s = c;
  S l{1, 2};
  auto [e, g] = l;
  [&] { constexpr int *lp = &g; }();
  thread_local auto [t, u] = gs;
  constexpr int *tp = &u;
}
constexpr const int *h() {
  constexpr auto [a, b] = gs;
  return &b;
}
constexpr const int *hp = h();
