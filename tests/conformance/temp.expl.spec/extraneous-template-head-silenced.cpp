// [temp.expl.spec]/17: -Wno-extraneous-template-head silences clang 22's
// extraneous 'template<>' diagnostic and its note; the member template
// definition is still used.
//@ requires: clang>=22
//@ args: -Wno-extraneous-template-head
//@ strict-diags
template<class T> struct O {};
template<> struct O<int> { template<char c> int f(); };
template<> template<char c> int O<int>::f() { return c; }
int use() { return O<int>().f<'a'>(); }
