// [class.dtor]/4, [dcl.decl.general]/5, [over.match.viable]/3: a trailing
// requires-clause on a non-templated function is ill-formed (clang
// err_constrained_non_templated_function), but the clause stays attached
// and is still checked with no template arguments: a plain class whose
// only destructor is unsatisfied has no viable destructor; of two
// prospective destructors only the satisfied one is viable (no ambiguity);
// a call or reference to an unsatisfied function fails. An explicit
// instantiation ([temp.explicit]) locates the specialization at its
// template-name before instantiating it, so the destructor error points
// there (clang ActOnExplicitInstantiation). Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:17 error "non-templated function cannot have a requires clause"
//@ diag: 38:8 error "no viable destructor found for class 'P'"
//@ diag: 39:3 note "candidate function not viable: constraints not satisfied"
//@ diag: 39:17 note "because 'false' evaluated to false"
//@ diag: 42:18 error "non-templated function cannot have a requires clause"
//@ diag: 43:18 error "non-templated function cannot have a requires clause"
//@ diag: 46:19 error "non-templated function cannot have a requires clause"
//@ diag: 47:12 error "no matching function for call to 'f'"
//@ diag: 46:6 note "candidate function not viable: constraints not satisfied"
//@ diag: 46:19 note "because 'false' evaluated to false"
//@ diag: 48:19 error "non-templated function cannot have a requires clause"
//@ diag: 51:13 error "invalid reference to function 'f': constraints not satisfied"
//@ diag: 46:19 note "because 'false' evaluated to false"
//@ diag: 57:17 error "no viable destructor found for class 'R<int>'"
//@ diag: 57:17 note "in instantiation of template class 'R<int>' requested here"
//@ diag: 54:3 note "candidate function not viable: constraints not satisfied"
//@ diag: 54:17 note "because 'false' evaluated to false"
//@ diag: 58:24 error "no viable destructor found for class 'R<long>'"
//@ diag: 58:24 note "in instantiation of template class 'R<long>' requested here"
//@ diag: 54:3 note "candidate function not viable: constraints not satisfied"
//@ diag: 54:17 note "because 'false' evaluated to false"
//@ diag: 59:17 error "no viable destructor found for class 'R<char>'"
//@ diag: 59:17 note "in instantiation of template class 'R<char>' requested here"
//@ diag: 54:3 note "candidate function not viable: constraints not satisfied"
//@ diag: 54:17 note "because 'false' evaluated to false"
struct P {
  ~P() requires false;
};
struct P2 {
  ~P2() requires false;
  ~P2() requires true;
};
P2 p2;
void f() requires false;
void g() { f(); }
void h() requires true;
void k() {
  h();
  auto p = &f;
}
template <class T> struct R {
  ~R() requires false {}
};
R<int> *pr;
template struct R<int>;
extern template struct R<long>;
template struct R<char>;
