// [temp.explicit]/3, [temp.expl.spec]/3 and [temp.pre]/2: an explicit
// instantiation or explicit specialization may appear only in namespace
// (or, for a specialization, class) scope. In a block clang 22 reads
// `template` without `<` as a missing template head ("expected '<' after
// 'template'" after the keyword) and `template<>` as a template-declaration
// ("templates can only be declared in namespace or class scope", or
// "function definition is not allowed here" for a definition). Clang 19
// gives "expected expression" (the -19 sibling) (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 19:12 error "expected '<' after 'template'"
//@ diag: 20:12 error "expected '<' after 'template'"
//@ diag: 21:3 error "templates can only be declared in namespace or class scope"
//@ diag: 22:28 error "function definition is not allowed here"
template<class T> struct S {};
template<class T> int g() { return 0; }
int f() {
  template struct S<long>;
  template int g<int>();
  template<> struct S<char> {};
  template<> int g<char>() { return 1; }
  return g<long>();
}
