// [temp.expl.spec], [temp.deduct.decl]: an explicit specialization that
// matches no template, that is ambiguous, or that follows an implicit
// instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:17 error "no function template matches function template specialization 'f'"
//@ diag: 17:24 note "candidate template ignored: failed template argument deduction"
//@ diag: 21:17 error "function template specialization 'g' ambiguously refers to more than one function template; explicitly specify template arguments to identify a particular function template"
//@ diag: 20:24 note "function template 'g<int>' matches specialization [with T = int]"
//@ diag: 19:24 note "function template 'g<int>' matches specialization [with T = int]"
//@ diag: 24:17 error "explicit specialization of 'u<int>' after instantiation"
//@ diag: 23:9 note "implicit instantiation first required here"
//@ diag: 26:17 error "no function template matches function template specialization 'v'"
//@ diag: 25:24 note "candidate template ignored: could not match 'int (int *)' against 'int (int)'"
//@ diag: 27:18 error "no function template matches function template specialization 'f'"
//@ diag: 17:24 note "candidate template ignored: could not match 'int' against 'void'"
template <class T> int f(T);
template <> int f(int, int);
template <class T> int g(T, int);
template <class T> int g(int, T);
template <> int g(int, int);
template <class T> int u(T) { return 0; }
int x = u(1);
template <> int u(int) { return 1; }
template <class T> int v(T*);
template <> int v<int>(int);
template <> void f(char);
