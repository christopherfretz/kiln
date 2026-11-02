// [temp.expl.spec]/17: the extraneous 'template<>' before a member template
// of an explicitly specialized class is an extension, not ill-formed past
// recovery; -Wno-error=extraneous-template-head downgrades clang 22's
// default error to a warning (clang 19 has no such group).
//@ requires: clang>=22
//@ args: -Wno-error=extraneous-template-head
//@ strict-diags
//@ diag: 14:1 warning "extraneous template parameter list in template specialization"
//@ diag: 13:19 note "'template<>' header not required for explicitly-specialized class 'O<int>' declared here"
//@ diag: 15:1 warning "extraneous template parameter list in template specialization"
//@ diag: 13:19 note "'template<>' header not required for explicitly-specialized class 'O<int>' declared here"
template<class T> struct O {};
template<> struct O<int> { template<char c> int f(); template<char c> static const int v; };
template<> template<char c> int O<int>::f() { return c; }
template<> template<char c> const int O<int>::v = c;
int use() { return O<int>().f<'a'>() + O<int>::v<'a'>; }
