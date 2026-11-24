// [temp.arg.general]/1, [temp.arg.type]/1, [temp.arg.template]/1: an
// elaborated-type-specifier naming a dependent class template-id
// (`struct It<T, T>`, `friend class It<T>;`) has its arguments checked
// against the template parameters as the simple-type-specifier
// `It<T, T>` does: the kind of each argument and the arity, with the
// same diagnostics at the same places (clang CheckTemplateArgumentList;
// WP-3322).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:9 error "template argument for non-type template parameter must be an expression"
//@ diag: 28:25 note "template parameter is declared here"
//@ diag: 32:16 error "template argument for non-type template parameter must be an expression"
//@ diag: 28:25 note "template parameter is declared here"
//@ diag: 33:3 error "too many template arguments for class template 'It'"
//@ diag: 28:35 note "template is declared here"
//@ diag: 34:9 error "too many template arguments for class template 'It'"
//@ diag: 28:35 note "template is declared here"
//@ diag: 35:3 error "too few template arguments for class template 'It'"
//@ diag: 28:35 note "template is declared here"
//@ diag: 36:10 error "too few template arguments for class template 'It'"
//@ diag: 28:35 note "template is declared here"
//@ diag: 37:8 error "template argument for template template parameter must be a class template or type alias template"
//@ diag: 38:15 error "template argument for template template parameter must be a class template or type alias template"
//@ diag: 39:10 error "too many template arguments for class template 'It'"
//@ diag: 28:35 note "template is declared here"
//@ diag: 40:16 error "too few template arguments for class template 'It'"
//@ diag: 28:35 note "template is declared here"
template <class T, bool B> struct It {};
template <class T, template <class> class TT> struct R {};
template <class T> struct C {
  It<T, T> *a1;
  struct It<T, T> *a2;
  It<T, 1, 2> *b1;
  class It<T, 1, 2> *b2;
  It<T> *c1;
  struct It<T> *c2;
  R<T, int> *d1;
  struct R<T, int> *d2;
  friend It<T, 1, 2>;
  friend class It<T>;
};
