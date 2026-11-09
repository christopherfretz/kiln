// [temp.spec.partial.order], [temp.func.order]/6.2.2: two partial
// specializations equally specialized by deduction are ordered by their
// constraints only when their template heads are equivalent and their
// arguments are the same (clang getMoreSpecialized); otherwise, or when
// neither is more constrained ([temp.constr.order]), the match is ambiguous
// (kiln used to pick the `D` one of `Y`; it reports `G` at `O`). WP-1053.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:9 error "ambiguous partial specializations of 'Y<int *, int *>'"
//@ diag: 24:50 note "partial specialization matches [with T = int, U = int]"
//@ diag: 25:50 note "partial specialization matches [with T = int, U = int]"
//@ diag: 32:9 error "ambiguous partial specializations of 'F<int *>'"
//@ diag: 30:41 note "partial specialization matches [with T = int]"
//@ diag: 31:52 note "partial specialization matches [with T = int]"
//@ diag: 39 error "ambiguous partial specializations of 'G<int *>'"
//@ diag: 36:43 note "partial specialization matches [with T = int]"
//@ diag: 37:54 note "partial specialization matches [with T = int]"
template <class T> concept C = sizeof(T) > 1;
template <class T> concept D = C<T> && sizeof(T) > 2;
// Equally specialized by deduction, but the argument lists differ: the
// constraints are not compared.
template <class T, class U> struct Y { static constexpr int v = 0; };
template <class T, class U> requires C<T> struct Y<T *, U *> { static constexpr int v = 1; };
template <class T, class U> requires D<T> struct Y<U *, T *> { static constexpr int v = 2; };
int y = Y<int *, int *>::v;
// Equivalent template heads and arguments, but neither constraint subsumes
// the other.
template <class T> struct F { static constexpr int v = 0; };
template <class T> requires C<T> struct F<T *> { static constexpr int v = 1; };
template <class T> requires (sizeof(T) > 1) struct F<T *> { static constexpr int v = 2; };
int f = F<int *>::v;
// The same in a member template of a class template.
template <class X> struct O {
  template <class T> struct G { static constexpr int v = 0; };
  template <class T> requires C<T> struct G<T *> { static constexpr int v = 1; };
  template <class T> requires (sizeof(T) > 1) struct G<T *> { static constexpr int v = 2; };
};
int g = O<int>::G<int *>::v;
