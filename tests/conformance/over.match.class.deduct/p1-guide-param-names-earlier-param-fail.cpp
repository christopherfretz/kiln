// [over.match.class.deduct]/1.1, [temp.deduct.general]/8: the implicit
// guide's later parameter type names its earlier parameter; for the deduced
// `T = int`, `t.g(1)` is ill-formed, so the guide's substitution fails (a
// substitution failure, not a lookup of `t` in `C2<int>`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:4 error "no viable constructor or deduction guide for deduction of template arguments of 'C2'"
//@ diag: 14:27 note "candidate template ignored: could not match 'C2<T>' against 'int'"
//@ diag: 14:27 note "implicit deduction guide declared as 'template <class T> C2(C2<T>) -> C2<T>'"
//@ diag: 14:32 note "candidate template ignored: substitution failure [with T = int]: member reference base type 'int' is not a structure or union"
//@ diag: 14:32 note "implicit deduction guide declared as 'template <class T> C2(T t, Arr<sizeof (t.g(1))> * = <null expr>) -> C2<T>'"
template <int N> struct Arr {};
struct S { int g(int); };
template <class T> struct C2 { C2(T t, Arr<sizeof(t.g(1))>* = nullptr) {} };
C2 c(1);
