// [temp.constr.order]/1, [temp.constr.normal]/1.4, [temp.arg.general]/9:
// the parameter mapping of a concept-id's atomic constraints is formed from
// its converted arguments, default template arguments included, so the
// atoms reached through `K<F, char*>` (third argument defaulted) and through
// `D<typename impl<char*, int>::type>` written directly are identical and
// the more constrained overload is chosen (libc++
// indirectly_comparable.compile.pass.cpp: `indirectly_comparable<I1, I2, R>`
// with its defaulted projections against `indirect_binary_predicate`).
//@ expect: compile-ok
//@ strict-diags
template <class T> concept C = sizeof(T) > 0;
template <class T> concept D = C<T*> && C<T&>;
template <class A, class B> struct impl { struct type {}; };
template <class A, class B> using proj = typename impl<A, B>::type;
template <class R, class I, class P = int> concept K = D<proj<I, P>>;
template <class R, class I, class P = I> concept KD = D<impl<I, P>>;

template <class F> requires K<F, char*> && true
constexpr int sub(F) { return 1; }
template <class F> requires D<proj<char*, int>>
constexpr int sub(F) { return 2; }
static_assert(sub(0) == 1);

template <class F> requires D<typename impl<char*, int>::type> && true
constexpr int rev(F) { return 1; }
template <class F> requires K<F, char*>
constexpr int rev(F) { return 2; }
static_assert(rev(0) == 1);

// A default argument naming an earlier parameter is substituted with the
// concept-id's arguments.
template <class F> requires KD<F, long> && true
constexpr int dep(F) { return 1; }
template <class F> requires D<impl<long, long>>
constexpr int dep(F) { return 2; }
static_assert(dep(0) == 1);
