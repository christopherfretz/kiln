// [temp.explicit]/5: the definition of a class template must be in scope
// at the point of its explicit instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:17 error "explicit instantiation of undefined template 'Fwd<int>'"
//@ diag: 7:27 note "template is declared here"
template <class T> struct Fwd;
template struct Fwd<int>;
template <class T> struct Def {};
template struct Def<int>;
