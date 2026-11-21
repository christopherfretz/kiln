//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): neither clang 22 nor GCC 16 implements P2686R5 constexpr references to automatic objects
// [expr.const.init]/2 and [dcl.struct.bind] (C++26, P2686R5): a binding of
// an automatic object names that object's member or element, which is
// constexpr-referenceable from a constexpr variable of the same function,
// so its address is a permitted result exactly as the member access it
// denotes (`&ly` as `&l.y`): by value, of an array, constexpr, by
// reference to a constexpr object, from a nested block. Also CWG 3135
// ([dcl.struct.bind]/7): a tuple-like binding whose get<i> returns a
// prvalue is an object of type T_i (not a reference to a temporary), read
// as that variable (a non-const T_i of a constexpr declaration, a const
// int constant-initialized without constexpr, usable as an array bound),
// and its address is constant even when it is not constant-initialized,
// at block scope and at namespace scope, also in a template. GCC 16.1
// accepts the CWG 3135 values; clang 22 rejects them.
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
struct Z {
  int m;
  template <std::size_t I> constexpr int get() const { return m + int(I); }
};
template <> struct std::tuple_size<Z> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, Z> { using type = int; };
struct N { template <std::size_t I> constexpr int get() const { return int(I) + 20; } };
template <> struct std::tuple_size<const N> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, const N> { using type = int; };

constexpr S gs{1, 2};
Z gz{1};

// A binding of an automatic object denotes that object's member or
// element, constexpr-referenceable from a constexpr variable of the same
// function: exactly as the member access it denotes.
void automatic() {
  S l{1, 2};
  auto [lx, ly] = l;
  constexpr int *p = &ly;
  constexpr int *q = &lx + 1;
  static_assert(p == q);
  constexpr int *m = &l.y;
  static_assert(m == &l.x + 1);
  int arr[2] = {3, 4};
  auto [a0, a1] = arr;
  constexpr int *pa = &a1;
  static_assert(pa == &a0 + 1);
  constexpr auto [ca, cb] = gs;
  constexpr const int *pc = &cb;
  constexpr const int &rc = ca;
  static_assert(*pc == 2 && rc == 1 && pc == &rc + 1);
  constexpr S cl{5, 6};
  constexpr auto &[ra, rb] = cl;
  static_assert(&rb == &cl.y && rb == 6);
  {
    constexpr const int *inner = &ly;
    static_assert(inner == p);
  }
}

// CWG 3135: a tuple-like binding whose get<i> returns a prvalue is an
// object of type T_i, not a reference to a temporary.
void prvalue_get() {
  constexpr auto [a, b] = N{};       // T_i is non-const int: still readable
  static_assert(a == 20 && b == 21);
  const auto [c, d] = X{};           // const int, constant-initialized
  static_assert(c == 10 && d == 11);
  int arr[d];
  static_assert(sizeof arr == 11 * sizeof(int));
  constexpr const int *pd = &d;
  static_assert(*pd == 11 && pd == &d);
  auto [e, f] = gz;                  // not constant-initialized: its address is
  constexpr int *pf = &f;
  static_assert(pf == &f && pf != &e);
}
auto [ge, gf] = gz;                  // a variable of static storage duration
constexpr int *pgf = &gf;
static_assert(pgf == &gf);

template <class U> int templ() {
  const auto [a, b] = U{};
  static_assert(b == 11);
  U u{};
  auto [c, d] = u;
  constexpr int *pd = &d;
  return *pd + b;
}
template int templ<X>();
