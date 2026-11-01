// [temp.explicit]/3: an explicit instantiation of a variable must name a static data member of a
// class template specialization or a variable template specialization
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:22 error "explicit instantiation of 'nope' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 11:14 error "explicit instantiation of 'w' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 10:5 note "explicit instantiation refers here"
template <class T> struct S { static T m; };
template int S<int>::nope;
int w;
template int w;
