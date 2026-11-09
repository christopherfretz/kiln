// [temp.pre]/2, [dcl.enum]: an enumeration is not itself a template;
// `template<...> enum E` (opaque or defined) is clang's "enumeration
// cannot be a template" after the name and enum-base, with "declaration
// does not declare anything" at `enum` (WP-1063).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:26 error "enumeration cannot be a template"
//@ diag: 15:19 error "declaration does not declare anything"
//@ diag: 16:37 error "enumeration cannot be a template"
//@ diag: 16:19 error "declaration does not declare anything"
//@ diag: 17:33 error "enumeration cannot be a template"
//@ diag: 17:19 error "declaration does not declare anything"
//@ diag: 19:32 error "enumeration cannot be a template"
//@ diag: 19:19 error "declaration does not declare anything"
template<class T> enum E {};
template<class T> enum class F : int;
template<class T> enum struct G { a, b };
struct S {
  template<int N> enum H : long;
};
