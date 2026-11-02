// [dcl.ambig.res]/1, [temp.res.general]/4-5: in `T f(X<T>::m);` at namespace
// or block scope the dependent qualified name is not implicitly a type, and
// a parenthesized expression is an initializer, so `f` is a variable (clang's
// isCXXFunctionDeclarator with the InvalidAsDeclSpec tie-break); a
// following decl-specifier or identifier (`T::m const a`, `T::m b, c`) makes
// it a declaration with a missing `typename`. At statement start a
// dependent template-id qualifier (`X<T>::m d, e;`) begins an expression
// statement, and recovery after "expected ';'" resumes at the next token.
// [basic.def]/5: an initialized `void` variable is reported when declared.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:25 error "variable has incomplete type 'void'"
//@ diag: 40:25 error "variable has incomplete type 'void'"
//@ diag: 41:27 error "missing 'typename' prior to dependent type name 'T::m'"
//@ diag: 42:27 error "missing 'typename' prior to dependent type name 'X<T>::m'"
//@ diag: 43:6 error "variable has incomplete type 'void'"
//@ diag: 47:8 error "variable has incomplete type 'void'"
//@ diag: 49:3 error "missing 'typename' prior to dependent type name 'T::m'"
//@ diag: 50:9 error "use of undeclared identifier 'r'"
//@ diag: 51:3 error "missing 'typename' prior to dependent type name 'T::m'"
//@ diag: 52:15 error "expected ';' after expression"
//@ diag: 53:10 error "expected ';' after expression"
//@ diag: 53:11 error "use of undeclared identifier 'd'"
//@ diag: 53:14 error "use of undeclared identifier 'e'"
//@ diag: 54:12 error "use of undeclared identifier 'p'"
//@ diag: 55:10 error "expected ';' after expression"
//@ diag: 55:17 error "a type specifier is required for all declarations"
//@ diag: 56:12 error "use of undeclared identifier 's'"
//@ diag: 57:12 error "expected ';' after expression"
//@ diag: 52:15 warning "expression result unused"
//@ diag: 57:12 warning "expression result unused"
//@ diag: 67:36 error "missing 'typename' prior to dependent type name 'X<T>::m'"
template <class T> struct X { using m = int; };

// Namespace scope: a parenthesized dependent qualified name that is not
// the start of a parameter-declaration makes the declarator a variable
// (clang's InvalidAsDeclSpec tie-break), here an invalid `void` one.
template <class T> void g1(X<T>::m);
template <class T> int g2(X<T>::m);
template <class T> void g3(T::m);
template <class T> int g4(T::m, int);
template <class T> int g5(X<T>::m) = delete;
void v1(1);

// Block scope.
template <class T> void f() {
  void h1(T::m);
  int h2(T::m);
  T::m const a = 0;
  T::m &r = a;
  T::m b, c;
  (void)(T::m)0;
  X<T>::m d, e;
  X<T>::m *p;
  X<T>::m const q = 0;
  X<T>::m &s = q;
  (X<T>::m)0;
}

// Contexts where `X<T>::m` is a type (but not `free_fn`'s parameter).
template <class T> struct Y {
  X<T>::m a;
  X<T>::m f(X<T>::m, int);
  using A = X<T>::m;
  void h() { typename X<T>::m z = 0; (void)z; int w(sizeof(X<T>)); (void)w; }
};
template <class T> X<T>::m free_fn(X<T>::m x) { return x; }
template <class T> auto k(T t) -> X<T>::m { return t; }
