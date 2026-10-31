// [temp.explicit]: an explicit instantiation must match a template, and
// an explicit instantiation after an explicit specialization has no
// effect; an explicit instantiation of an undefined function template.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:14 error "explicit instantiation of 'f' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 11:24 note "candidate template ignored: failed template argument deduction"
//@ diag: 14:22 error "explicit instantiation of 'g' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 17:14 warning "explicit instantiation of 'k<int>' that occurs after an explicit specialization has no effect"
//@ diag: 16:17 note "previous template specialization is here"
template <class T> int f(T);
template int f(int, int);
template <class T> struct A { int f(); };
template int A<int>::g();
template <class T> int k(T) { return 0; }
template <> int k(int) { return 1; }
template int k(int);
