// [temp.res.general]/6, [temp.arg.general]/1: a class template-id with
// dependent arguments is checked at its definition for what does not
// depend on them (clang's `CheckTemplateArgumentList` on dependent
// arguments): the arity and each argument's kind.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 45:3 error "too many template arguments for class template 'A'"
//@ diag: 35:27 note "template is declared here"
//@ diag: 46:5 error "template argument for template template parameter must be a class template or type alias template"
//@ diag: 47:5 error "template argument for non-type template parameter must be an expression"
//@ diag: 37:15 note "template parameter is declared here"
//@ diag: 48:5 error "use of template template parameter 'TT' requires template arguments"
//@ diag: 44:43 note "template is declared here"
//@ diag: 49:5 error "template argument for non-type template parameter must be an expression"
//@ diag: 37:15 note "template parameter is declared here"
//@ diag: 51:3 error "too few template arguments for class template 'Q'"
//@ diag: 39:42 note "template is declared here"
//@ diag: 52:3 error "too many template arguments for class template 'Q'"
//@ diag: 39:42 note "template is declared here"
//@ diag: 57:31 error "too many template arguments for class template 'A'"
//@ diag: 35:27 note "template is declared here"
//@ diag: 57:44 error "template argument for template template parameter must be a class template or type alias template"
//@ diag: 62:5 error "template argument for template type parameter must be a type; did you forget 'typename'?"
//@ diag: 35:17 note "template parameter is declared here"
//@ diag: 63:5 error "template argument for template template parameter must be a class template or type alias template"
//@ diag: 66:5 error "template argument for non-type template parameter must be an expression"
//@ diag: 37:15 note "template parameter is declared here"
//@ diag: 67:5 error "template argument for template type parameter must be a type; did you forget 'typename'?"
//@ diag: 38:17 note "template parameter is declared here"
//@ diag: 67:11 error "template argument for template type parameter must be a type; did you forget 'typename'?"
//@ diag: 38:26 note "template parameter is declared here"
//@ diag: 75:3 error "too many template arguments for class template 'P'"
//@ diag: 38:36 note "template is declared here"
//@ diag: 76:5 error "template argument for template template parameter must be a class template or type alias template"
template <class T> struct A {};
template <template <class> class TT> struct B {};
template <int N> struct I {};
template <class T, class U> struct P {};
template <class T, class U = int> struct Q {};
template <class... Ts> struct V {};

// Checked when the template is defined: the arity and the kind of each
// argument, whatever the template parameters are replaced by.
template <class T, template <class> class TT> struct S {
  A<T, int> a1;
  B<T> b1;
  I<TT> i1;
  A<TT> a2;
  I<T> i2;
  Q<T> q1;
  Q<> q2;
  Q<T, T, T> q3;
  B<A> ok1;
  B<TT> ok2;
  V<T, TT<T>> ok3;
};
template <class T> void f() { A<T, T> x; B<T> y; }

// A dependent `T::x` for a type parameter is reported and the check
// goes on to the next argument.
template <class T> struct D {
  A<T::x> a;
  B<T::x> b;
  B<T::template y> ok1;
  I<T::x> ok2;
  I<typename T::x> i;
  P<T::x, T::y> p;
};

// A pack expansion may cover any number of parameters: the arguments
// before it are checked, and the arity only as far as they go.
template <class... Ts> struct E { A<Ts...> a; Q<int, Ts...> q; };
template <class... Ts> struct F {
  P<Ts..., int, int, int> ok;
  P<int, int, int, Ts...> p;
  B<int, Ts...> b;
};
