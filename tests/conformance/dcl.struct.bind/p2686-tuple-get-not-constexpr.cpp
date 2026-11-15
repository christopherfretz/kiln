//@ expect: compile-fail
//@ strict-diags
//@ oracle: gcc
//@ xfail(clang): clang 22 does not check the holding variables of a constexpr structured binding
//@ diag: 56:18 error "constexpr variable 'x' must be initialized by a constant expression"
//@ diag: 56:18 note "in implicit initialization of binding declaration 'x'"
//@ diag: 56:17 note "non-constexpr function 'get<0>' cannot be used in a constant expression"
//@ diag: 44:40 note "declared here"
//@ diag: 56:21 error "constexpr variable 'y' must be initialized by a constant expression"
//@ diag: 56:21 note "in implicit initialization of binding declaration 'y'"
//@ diag: 56:17 note "non-constexpr function 'get<1>' cannot be used in a constant expression"
//@ diag: 44:40 note "declared here"
//@ diag: 57:18 error "variable does not have a constant initializer"
//@ diag: 57:18 note "in implicit initialization of binding declaration 'u'"
//@ diag: 57:1 note "required by 'constinit' specifier here"
//@ diag: 57:17 note "non-constexpr function 'get<0>' cannot be used in a constant expression"
//@ diag: 44:40 note "declared here"
//@ diag: 57:21 error "variable does not have a constant initializer"
//@ diag: 57:21 note "in implicit initialization of binding declaration 'v'"
//@ diag: 57:1 note "required by 'constinit' specifier here"
//@ diag: 57:17 note "non-constexpr function 'get<1>' cannot be used in a constant expression"
//@ diag: 44:40 note "declared here"
//@ diag: 64:20 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 64:20 note "in implicit initialization of binding declaration 'p'"
//@ diag: 64:19 note "non-constexpr function 'get<0>' cannot be used in a constant expression"
//@ diag: 44:40 note "declared here"
//@ diag: 64:23 error "constexpr variable 'q' must be initialized by a constant expression"
//@ diag: 64:23 note "in implicit initialization of binding declaration 'q'"
//@ diag: 64:19 note "non-constexpr function 'get<1>' cannot be used in a constant expression"
//@ diag: 44:40 note "declared here"
// [dcl.struct.bind]/1, /7 (C++26, P2686R5): the variables r_i that hold a
// tuple-like binding are declared with the declaration's constexpr or
// constinit, so each is constant-initialized from get<i>(e): a get that is
// not constexpr makes the declaration ill-formed, at namespace and block
// scope. clang 22 does not check them; GCC 16.1 does (kiln follows the
// standard text). A constexpr get is fine. Found running GCC's C++
// testsuite.
namespace std {
template <class T> struct tuple_size;
template <decltype(sizeof 0) I, class T> struct tuple_element;
}
struct A {
  int i, j;
  template <decltype(sizeof 0) I> int &get() { return I ? j : i; }
};
template <> struct std::tuple_size<A> { static const int value = 2; };
template <decltype(sizeof 0) I> struct std::tuple_element<I, A> { using type = int; };
struct C {
  int i, j;
  template <decltype(sizeof 0) I> constexpr const int &get() const { return I ? j : i; }
};
template <> struct std::tuple_size<const C> { static const int value = 2; };
template <decltype(sizeof 0) I> struct std::tuple_element<I, const C> { using type = const int; };

A a{1, 2};
constexpr auto &[x, y] = a;
constinit auto &[u, v] = a;
constexpr C c{3, 4};
constexpr auto [m, n] = c;
static_assert(m + n == 7);

int f() {
  static A sa{5, 6};
  constexpr auto &[p, q] = sa;
  return p + q;
}
