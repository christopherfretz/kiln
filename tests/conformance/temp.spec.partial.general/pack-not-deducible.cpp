// [temp.spec.partial.general]/8 with packs: every partial specialization
// parameter must be deducible. A pack expansion that is not the last argument
// makes the whole template argument list a non-deduced context
// ([temp.deduct.type]/9), and so does an expansion of an expression
// ([temp.deduct.type]/5.3); a function parameter pack that is not last is a
// non-deduced context but the parameters after it are still deduced
// ([temp.deduct.type]/5.7). Found by the clang negative-test run (WP-912),
// fixed by WP-956.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:33 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 23:18 note "non-deducible template parameter 'Values'"
//@ diag: 26:46 error "class template partial specialization contains template parameters that cannot be deduced; this partial specialization will never be used"
//@ diag: 26:20 note "non-deducible template parameter 'Types'"
//@ diag: 26:33 note "non-deducible template parameter 'Tail'"
//@ diag: 28:39 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 28:20 note "non-deducible template parameter 'T'"
//@ diag: 29:39 error "class template partial specialization contains template parameters that cannot be deduced; this partial specialization will never be used"
//@ diag: 29:20 note "non-deducible template parameter 'T'"
//@ diag: 29:29 note "non-deducible template parameter 'V'"
template <int... Values> struct X1;
template <int... Values> struct X1<0, Values + 1 ...>;
template <class... T> struct L {};
template <class... Types> struct U;
template <class... Types, class Tail> struct U<Types..., Tail>;
template <class T> struct Y;
template <class... T, class V> struct Y<void(T..., V)> {};
template <class... T, class V> struct Y<L<T..., V>>;
template <class... T, class V> struct Y<void(T..., V, T...)> {};
template <class... T, class V> struct Y<L<V, T...> *> {};
Y<L<int, char> *> y2;
