// [basic.def.odr]/1, [temp.expl.spec]: an explicit specialization is a
// definition and may be defined only once.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:20 error "redefinition of 'A<int>'"
//@ diag: 10:20 note "previous definition is here"
//@ diag: 14:27 error "redefinition of 'w<int>'"
//@ diag: 13:27 note "previous definition is here"
template <class T> struct A {};
template <> struct A<int> {};
template <> struct A<int> {};
template <class T> constexpr int w = 0;
template <> constexpr int w<int> = 1;
template <> constexpr int w<int> = 2;
