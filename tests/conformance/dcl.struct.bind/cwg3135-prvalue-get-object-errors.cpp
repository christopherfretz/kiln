//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 implements neither CWG 3135 nor P2686R5 references to automatic objects
//@ diag: 55:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:15 note "read of non-constexpr variable 'q' is not allowed in a constant expression"
//@ diag: 54:16 note "declared here"
//@ diag: 58:17 error "static assertion expression is not an integral constant expression"
//@ diag: 58:17 note "read of non-const variable 'b' is not allowed in a constant expression"
//@ diag: 57:12 note "declared here"
//@ diag: 61:17 error "static assertion expression is not an integral constant expression"
//@ diag: 61:17 note "initializer of 'd' is not a constant expression"
//@ diag: 60:18 note "declared here"
//@ diag: 63:17 error "static assertion expression is not an integral constant expression"
//@ diag: 63:17 note "initializer of 'd' is not a constant expression"
//@ diag: 60:18 note "declared here"
//@ diag: 65:31 error "constexpr variable 'pr' must be initialized by a constant expression"
//@ diag: 65:31 note "pointer to 'r' is not a constant expression"
//@ diag: 64:24 note "address of non-static constexpr variable 'r' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 67:31 error "constexpr variable 'rg' must be initialized by a constant expression"
//@ diag: 67:31 note "reference to 'g' is not a constant expression"
//@ diag: 66:22 note "declared here"
// [dcl.struct.bind]/7 (CWG 3135) and [expr.const.init]: a tuple-like
// binding whose get<i> returns a prvalue is an object of type T_i, usable
// in constant expressions only when potentially-constant (constexpr, or
// const integral) and constant-initialized: a const class object, a
// non-const int, and a const int whose get reads a non-constexpr object
// are not read (its address still is constant). [expr.const.init]/2,4: a
// lifetime-extended temporary of a constexpr local reference or binding is
// automatic, not constexpr-referenceable from a static variable's
// initializer (the notes name the variable, as clang's for the reference).
// GCC 16.1 rejects the same reads.
namespace std {
using size_t = decltype(sizeof 0);
template <class T> struct tuple_size;
template <size_t I, class T> struct tuple_element;
template <class T> requires requires { tuple_size<T>::value; }
struct tuple_size<const T> { static constexpr size_t value = tuple_size<T>::value; };
template <size_t I, class T> struct tuple_element<I, const T> { using type = const typename tuple_element<I, T>::type; };
} // namespace std
struct Pt { int v; };
struct X { template <std::size_t I> constexpr int get() const { return int(I) + 10; } };
template <> struct std::tuple_size<X> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, X> { using type = int; };
struct Y { template <std::size_t I> constexpr Pt get() const { return {int(I) + 20}; } };
template <> struct std::tuple_size<Y> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, Y> { using type = Pt; };
struct Z {
  int m;
  template <std::size_t I> constexpr int get() const { return m + int(I); }
};
template <> struct std::tuple_size<Z> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, Z> { using type = int; };

const auto [p, q] = Y{};
static_assert(q.v == 21);
void f() {
  auto [a, b] = X{};
  static_assert(b == 11);
  Z z{1};
  const auto [c, d] = z;
  static_assert(d == 2);
  constexpr const int *pd = &d;
  static_assert(*pd == 2);
  constexpr const int &r = 11;
  static constexpr const int *pr = &r;
  constexpr auto [e, g] = X{};
  static constexpr const int &rg = g;
}
