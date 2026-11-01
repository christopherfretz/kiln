// [temp.expl.spec]/16, [temp.mem]/1: a specialized member template must
// name a member template of the class; one only declared leaves its
// specializations undefined (the note names that declaration).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:51 error "out-of-line definition of 'Nope' does not match any declaration in 'Outer<short>'"
//@ diag: 14:54 error "out-of-line definition of 'Nope' does not match any declaration in 'Outer<T>'"
//@ diag: 17:25 error "implicit instantiation of undefined template 'Outer<char>::Inner<int>'"
//@ diag: 15:50 note "template is declared here"
template<class T> struct Outer {
  template<class U> struct Inner { static constexpr int v = 1; };
};
template<> template<class U> struct Outer<short>::Nope { };
template<class T> template<class U> struct Outer<T>::Nope { };
template<> template<class U> struct Outer<char>::Inner;
Outer<char>::Inner<int> *p;
Outer<char>::Inner<int> q;
