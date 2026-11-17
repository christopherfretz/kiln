// [temp.constr.normal]/1.4, [temp.constr.atomic]/1-3: the arguments of a
// concept-id include the concept's default arguments
// ([temp.arg.general]/9), and they are substituted only into the parameter
// mappings of the concept's atoms. `true` names no parameter, so its
// mapping is empty: `C<int>` reached through a constraint (`C auto`,
// `template <C T>`, `requires C<T>`) is satisfied although the default
// `typename T::type` is invalid for `int`. GCC 16.1 agrees; clang 19 and
// 22 substitute the default argument and reject (the text decides).
// Naming `C<int>` directly outside a constraint forms the template-id
// itself, which all three reject. Found running GCC's C++ testsuite.
//@ std: c++20
//@ oracle: gcc
//@ skip(clang): clang 19/22 substitute a concept's default argument outside the parameter mapping
//@ strict-diags
template <typename T, typename U = typename T::type> concept C = true;
constexpr bool f(C auto) { return true; }
static_assert(f(0));
C auto x = 0;
template <C T> constexpr int g(T) { return 1; }
static_assert(g(0) == 1);
template <class T> requires C<T> constexpr int g2(T) { return 1; }
static_assert(g2(0) == 1);

// A member of a class template, checked when it is used.
template <class T> struct S {
  static constexpr int m() requires C<T> { return 1; }
};
static_assert(S<int>::m() == 1);

// A nested requirement ([expr.prim.req.nested]/1: satisfied as its
// constraint-expression is), with a defaulted or an explicit argument.
template <class T> concept N = requires { requires C<T>; };
static_assert(N<int>);
template <class T> concept Two = true;
template <class T> concept N2 = requires { requires Two<typename T::nope>; };
static_assert(N2<int>);
// A return-type-requirement ([expr.prim.req.compound]/1.3.2).
template <class T> concept One = Two<typename T::type>;
template <class T> concept Zero = requires { { T() } -> One; };
static_assert(Zero<int>);

// A default that names an earlier parameter used by an atom.
template <typename T, typename U = typename T::type> concept D = sizeof(U) > 0;
template <D T> constexpr int h(T) { return 1; }
constexpr int h(...) { return 2; }
struct HasType { using type = int; };
static_assert(h(0) == 2);
static_assert(h(HasType{}) == 1);
template <class T> concept ND = requires { requires D<T>; };
static_assert(!ND<int> && ND<HasType>);
