// [over.match.class.deduct]/3 (P1814): the guides of an alias template
// as clang prints them (`requires __is_deducible(A, rhs)`, declared at
// the alias's `using`); a guide whose return type does not deduce the
// alias's parameters fails its deducibility constraint; an alias whose
// right-hand side is not a template specialization is not deducible.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 50:6 error "no viable constructor or deduction guide for deduction of template arguments of 'Same'"
//@ diag: 49:19 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'double')"
//@ diag: 49:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(Same, P<T, T>) Same(T, T) -> P<T, T>'"
//@ diag: 49:19 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 49:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(Same, P<T, T>) Same(P<T, T>) -> P<T, T>'"
//@ diag: 52:5 error "no viable constructor or deduction guide for deduction of template arguments of 'Ptr'"
//@ diag: 51:19 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 51:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(Ptr, P<T *, T>) Ptr(T *, T) -> P<T *, T>'"
//@ diag: 51:19 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 51:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(Ptr, P<T *, T>) Ptr(P<T *, T>) -> P<T *, T>'"
//@ diag: 55:4 error "no viable constructor or deduction guide for deduction of template arguments of 'QQ'"
//@ diag: 54:19 note "candidate template ignored: constraints not satisfied"
//@ diag: 54:19 note "cannot deduce template arguments for 'QQ' from 'Q<int>'"
//@ diag: 54:19 note "implicit deduction guide declared as 'template <> requires __is_deducible(QQ, Q<int>) QQ(int) -> Q<int>'"
//@ diag: 54:19 note "candidate template ignored: constraints not satisfied"
//@ diag: 54:19 note "cannot deduce template arguments for 'QQ' from 'Q<int>'"
//@ diag: 54:19 note "implicit deduction guide declared as 'template <> requires __is_deducible(QQ, Q<int>) QQ(Q<int>) -> Q<int>'"
//@ diag: 57:5 error "alias template 'Bad' requires template arguments; argument deduction only allowed for class templates or alias templates"
//@ diag: 56:19 note "template is declared here"
//@ diag: 60:3 error "no viable constructor or deduction guide for deduction of template arguments of 'W'"
//@ diag: 59:19 note "candidate template ignored: couldn't infer template argument 'T'"
//@ diag: 59:19 note "implicit deduction guide declared as 'template <class T, class It> requires __is_deducible(W, V<T>) W(It, It) -> V<T>'"
//@ diag: 59:19 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 59:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(W, V<T>) W(T) -> V<T>'"
//@ diag: 59:19 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 59:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(W, V<T>) W(V<T>) -> V<T>'"
//@ diag: 64:4 error "no matching constructor for initialization of 'G<long>'"
//@ diag: 61:26 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'const char[2]' to 'const G<long>' for 1st argument"
//@ diag: 61:26 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'const char[2]' to 'G<long>' for 1st argument"
//@ diag: 61:30 note "candidate constructor not viable: no known conversion from 'const char[2]' to 'long' for 1st argument"
//@ diag: 67:4 error "no viable constructor or deduction guide for deduction of template arguments of 'CA'"
//@ diag: 66:17 note "candidate template ignored: could not match 'char (*)[N]' against 'std::nullptr_t'"
//@ diag: 66:17 note "implicit deduction guide declared as 'template <int N> requires __is_deducible(CA, Arr<char, N>) CA(char (*)[N]) -> Arr<char, N>'"
//@ diag: 66:17 note "candidate template ignored: could not match 'Arr<char, N>' against 'std::nullptr_t'"
//@ diag: 66:17 note "implicit deduction guide declared as 'template <int N> requires __is_deducible(CA, Arr<char, N>) CA(Arr<char, N>) -> Arr<char, N>'"
//@ diag: 70:4 error "no viable constructor or deduction guide for deduction of template arguments of 'TI'"
//@ diag: 69:23 note "candidate function template not viable: no known conversion from 'std::nullptr_t' to 'int' for 1st argument"
//@ diag: 69:23 note "implicit deduction guide declared as 'template <class ...Us> requires __is_deducible(TI, Tup<int, Us...>) TI(int, Us ...) -> Tup<int, Us...>'"
//@ diag: 69:23 note "candidate template ignored: could not match 'Tup<int, Us...>' against 'std::nullptr_t'"
//@ diag: 69:23 note "implicit deduction guide declared as 'template <class ...Us> requires __is_deducible(TI, Tup<int, Us...>) TI(Tup<int, Us...>) -> Tup<int, Us...>'"
template<class T, class U> struct P { P(T, U); };
template<class T> using Same = P<T, T>;
Same s(1, 2.0);
template<class T> using Ptr = P<T*, T>;
Ptr pp(1, 1);
template<class T> struct Q { Q(T); };
template<class T> using QQ = Q<int>;
QQ q(1);
template<class T> using Bad = T;
Bad b(1);
template<class T> struct V { V(T); template<class It> V(It, It); };
template<class T> using W = V<T>;
W w2((int*)0, (int*)0);
template<class T> struct G { G(T); };
G(const char *) -> G<long>;
template<class T> using GA = G<T>;
GA ga("x");
template<class T, int N> struct Arr { Arr(T (*)[N]); };
template<int N> using CA = Arr<char, N>;
CA cb(nullptr);
template<class... Ts> struct Tup { Tup(Ts...); };
template<class... Us> using TI = Tup<int, Us...>;
TI ti(nullptr);
