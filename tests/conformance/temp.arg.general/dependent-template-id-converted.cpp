// [temp.arg.general]/1, [temp.res.general]/6: in a template-id with
// dependent arguments a non-dependent non-type argument is converted at the
// definition, a missing argument takes its default (substituted with the
// dependent prior arguments, checked "while checking a default template
// argument"), and a variable template-id or concept-id is checked for its
// arity as a class template-id is (clang's `CheckTemplateArgumentList`).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 45:32 error "value of type 'int' is not implicitly convertible to 'int *'"
//@ diag: 40:24 note "template parameter is declared here"
//@ diag: 46:33 error "non-type template argument evaluates to 1000, which cannot be narrowed to type 'char'"
//@ diag: 47:33 error "conversion from 'double' to 'int' is not allowed in a converted constant expression"
//@ diag: 42:23 note "template parameter is declared here"
//@ diag: 48:33 error "value of type 'int *' is not implicitly convertible to 'int'"
//@ diag: 42:23 note "template parameter is declared here"
//@ diag: 49:33 error "non-type template argument is not a constant expression"
//@ diag: 49:33 note "read of non-const variable 'g' is not allowed in a constant expression"
//@ diag: 43:5 note "declared here"
//@ diag: 50:32 error "conversion from 'int' to 'int *' is not allowed in a converted constant expression"
//@ diag: 40:24 note "template parameter is declared here"
//@ diag: 53:28 error "value of type 'int' is not implicitly convertible to 'int *'"
//@ diag: 54:31 note "while checking a default template argument used here"
//@ diag: 53:24 note "template parameter is declared here"
//@ diag: 55:35 error "value of type 'int' is not implicitly convertible to 'int *'"
//@ diag: 56:34 note "while checking a default template argument used here"
//@ diag: 55:31 note "template parameter is declared here"
//@ diag: 57:43 error "value of type 'int' is not implicitly convertible to 'int *'"
//@ diag: 58:31 note "while checking a default template argument used here"
//@ diag: 57:39 note "template parameter is declared here"
//@ diag: 59:56 error "conversion from 'int' to 'int *' is not allowed in a converted constant expression"
//@ diag: 60:31 note "while checking a default template argument used here"
//@ diag: 59:52 note "template parameter is declared here"
//@ diag: 64:37 error "too many template arguments for variable template 'vt'"
//@ diag: 62:33 note "template is declared here"
//@ diag: 65:38 error "too many template arguments for concept 'Co'"
//@ diag: 63:27 note "template is declared here"
//@ diag: 67:45 error "pack expansion used as argument for non-pack parameter of concept"
//@ diag: 63:16 note "template parameter is declared here"
template<class T, int *P> struct B;
template<class T, char C> struct Cc;
template<class T, int N> struct In;
int g;
// A non-dependent argument is converted.
template<class T> void f1(B<T, 1>);
template<class T> void f2(Cc<T, 1000>);
template<class T> void f3(In<T, 1.5>);
template<class T> void f4(In<T, (int *)0>);
template<class T> void f5(In<T, g>);
template<class T> void f6(B<T, 0>);
template<class T> void f7(B<T, nullptr>, Cc<T, 'a'>, In<T, sizeof(T) + 1>);
// A missing argument takes its default, substituted with the prior ones.
template<class T, int *P = 1> struct D1;
template<class T> void h1(D1<T>);
template<class T, int N, int *P = N> struct D2;
template<class T> void h2(D2<T, 1>);
template<class T, class U = int, int *P = int(sizeof(U))> struct D3;
template<class T> void h3(D3<T>);
template<class T, class U = typename T::type, int *P = int(sizeof(U))> struct D4;
template<class T> void h4(D4<T>);
// Variable templates and concepts with dependent arguments.
template<class T> constexpr int vt = 0;
template<class T> concept Co = true;
template<class T> int k1() { return vt<T, int>; }
template<class T> bool k2() { return Co<T, int>; }
template<class... Ts> int k3() { return vt<Ts...>; }
template<class... Ts> bool k4() { return Co<Ts...>; }
