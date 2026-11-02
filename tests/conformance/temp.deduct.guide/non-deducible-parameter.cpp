// [temp.deduct.guide]/3: every template parameter of a deduction-guide
// template is deducible from the guide's parameter types ([temp.deduct.type]/5
// non-deduced contexts do not count), is a pack, or has a default argument
// (clang `err_deduction_guide_template_not_deducible`, one note per
// parameter).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:20 error "deduction guide template contains a template parameter that cannot be deduced"
//@ diag: 22:17 note "non-deducible template parameter 'T'"
//@ diag: 24:29 error "deduction guide template contains template parameters that cannot be deduced"
//@ diag: 24:17 note "non-deducible template parameter 'T'"
//@ diag: 24:26 note "non-deducible template parameter 'U'"
//@ diag: 29:18 error "deduction guide template contains a template parameter that cannot be deduced"
//@ diag: 29:15 note "non-deducible template parameter 'N'"
//@ diag: 31:20 error "deduction guide template contains a template parameter that cannot be deduced"
//@ diag: 31:17 note "non-deducible template parameter 'T'"
//@ diag: 34:22 error "deduction guide template contains a template parameter that cannot be deduced"
//@ diag: 34:19 note "non-deducible template parameter 'T'"
template <class T> struct X { using m = int; };
template <class T> struct S { S(int); };

template <class T> S(typename X<T>::m*) -> S<T>;
template <class T> S(typename X<T>::m*, T) -> S<T>;
template <class T, class U> S(typename X<T>::m, typename X<U>::m) -> S<T>;
template <class T, class U = int> S(typename X<T>::m, T, char) -> S<T>;
template <class... T> S(X<int>::m, int, char) -> S<int>;
template <class T> S(T (*)[sizeof(T)]) -> S<T>;
template <int N> S(int (&)[N]) -> S<int>;
template <int N> S(int (&)[N + 1], char) -> S<int>;
template <template <class> class TT> S(TT<int>) -> S<int>;
template <class T> S(int, long) -> S<T>;
struct G {
  template <class T> struct W { W(int); };
  template <class T> W(X<T>::m) -> W<T>;
};
