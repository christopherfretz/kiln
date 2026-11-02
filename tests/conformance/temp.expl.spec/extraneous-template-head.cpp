// [temp.expl.spec]/17, [temp.mem]/1: a member template of an explicitly
// specialized class is defined out of line with exactly the member's own
// template head; a leading 'template<>' for the already-specialized class
// is extraneous. Clang 22 makes it a default-error extension under
// -Wextraneous-template-head (with a note at the explicit specialization);
// an extra non-empty head is always an error. Covers a member function
// template, member class template, static data member template, a member
// specialization and explicit specializations at depth 2.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:1 error "extraneous template parameter list in template specialization"
//@ diag: 31:19 note "'template<>' header not required for explicitly-specialized class 'O<int, char>' declared here"
//@ diag: 38:1 error "extraneous template parameter list in template specialization"
//@ diag: 31:19 note "'template<>' header not required for explicitly-specialized class 'O<int, char>' declared here"
//@ diag: 39:1 error "extraneous template parameter list in template specialization"
//@ diag: 31:19 note "'template<>' header not required for explicitly-specialized class 'O<int, char>' declared here"
//@ diag: 40:1 error "extraneous template parameter list in template specialization"
//@ diag: 31:19 note "'template<>' header not required for explicitly-specialized class 'O<int, char>' declared here"
//@ diag: 41:1 error "extraneous template parameter list in template specialization"
//@ diag: 31:19 note "'template<>' header not required for explicitly-specialized class 'O<int, char>' declared here"
//@ diag: 50:1 error "extraneous template parameter list in template specialization"
//@ diag: 49:27 note "'template<>' header not required for explicitly-specialized class 'A<int>::B<int>' declared here"
//@ diag: 51:1 error "extraneous template parameter list in template specialization"
//@ diag: 45:19 note "'template<>' header not required for explicitly-specialized class 'A<int>::M' declared here"
//@ diag: 52:23 error "extraneous template parameter list in template specialization"
//@ diag: 53:1 error "extraneous template parameter list in template specialization"
//@ diag: 54:1 error "extraneous template parameter list in template specialization or out-of-line template definition"
//@ diag: 31:19 note "'template<>' header not required for explicitly-specialized class 'O<int, char>' declared here"
template<class T, class U> struct O {};
template<> struct O<int, char> {
  template<char c> int f();
  template<char c> struct C;
  template<char c> static const int v;
  template<class X> struct N;
};
template<> template<char c> int O<int, char>::f() { return c; }
template<> template<char c> struct O<int, char>::C { static const int k = c; };
template<> template<char c> const int O<int, char>::v = c + 1;
template<> template<> int O<int, char>::f<'b'>() { return 0; }
template<> template<class X> struct O<int, char>::N { int g() { return 4; } };
template<class T> struct A {
  template<class U> struct B { template<class V> int h() { return 0; } };
};
template<> struct A<int> {
  template<class U> struct B { template<class Z> int h(); };
  struct M { template<class Z> int k(); };
};
template<> struct A<int>::B<int> { template<class Z> int h(); };
template<> template<> template<class Z> int A<int>::B<int>::h() { return 1; }
template<> template<class Z> int A<int>::M::k() { return 2; }
template<> template<> template<> template<class Z> int A<long>::B<int>::h() { return 3; }
template<> template<class T> void free_f(T);
template<> template<class T> template<char c> int O<int, char>::f() { return 0; }
int use() {
  return O<int, char>().f<'a'>() + O<int, char>::C<'a'>::k + O<int, char>::v<'a'>
       + O<int, char>::N<int>().g() + A<int>::B<int>().h<int>() + A<int>::M().k<int>()
       + A<long>::B<int>().h<int>();
}
