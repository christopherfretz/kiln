// [temp.over.link]/5, /7: a function template redeclared with the same
// instantiation-dependent array bound (its value known, its expression
// naming `T`) declares one template; one whose bound differs only in such
// an expression, or writes its value, is functionally equivalent but not
// equivalent, which is ill-formed, no diagnostic required: kiln, like
// clang, treats it as the same template (the constant array's canonical
// type is its value). A
// value-dependent bound (`sizeof(f(T()))`) is compared by its
// expression: two spellings are two templates, and a call that both
// match is ambiguous.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:9 error "call to 'k' is ambiguous"
//@ diag: 25:24 note "candidate function [with T = int]"
//@ diag: 26:24 note "candidate function [with T = int]"
struct X {};
constexpr int f(int) { return 0; }
constexpr int f3(int) { return 0; }

template <class T> int h(char (*)[sizeof(sizeof(f(T())))]);
template <class T> int h(char (*)[sizeof(sizeof(f(T())))]);
template <class T> int h(char (*)[sizeof(sizeof(0))]);
int z = h<int>(nullptr);

template <class T> int k(char (*)[sizeof(f(T()))]);
template <class T> int k(char (*)[sizeof(f3(T()))]);
int y = k<int>(nullptr);
