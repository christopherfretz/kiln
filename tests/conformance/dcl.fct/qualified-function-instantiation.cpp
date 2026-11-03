// [dcl.meaning.general]/5, [temp.inst]/17 (clang `VisitFieldDecl` /
// `VisitVarDecl`): a data member, a variable or a static data member whose
// type substitutes to a function type, qualified or not, is an error at its
// name in the instantiation. A member function's parameter of that type is
// not diagnosed (clang accepts it; it adjusts to a pointer). Matches clang
// 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:33 error "data member instantiated with function type 'int () const'"
//@ diag: 21:6 note "in instantiation of template class 'W<int () const>' requested here"
//@ diag: 20:33 error "data member instantiated with function type 'int ()'"
//@ diag: 22:10 note "in instantiation of template class 'W<int ()>' requested here"
//@ diag: 23:33 error "variable instantiated with function type 'int () const'"
//@ diag: 24:15 note "in instantiation of function template specialization 'k<int () const>' requested here"
//@ diag: 23:33 error "variable instantiated with function type 'int (int)'"
//@ diag: 25:15 note "in instantiation of function template specialization 'k<int (int)>' requested here"
//@ diag: 26:40 error "static data member instantiated with function type 'int () const'"
//@ diag: 27:6 note "in instantiation of template class 'Z<int () const>' requested here"
typedef int F() const;
template <class T> struct W { T t; };
W<F> w;
W<int()> w2;
template <class T> void k() { T loc; }
template void k<F>();
template void k<int(int)>();
template <class T> struct Z { static T s; void m(T p); };
Z<F> z;
