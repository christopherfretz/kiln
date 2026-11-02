// [temp.names]/9, [temp.constr.decl]/3: a non-dependent template-id naming
// a specialization of an alias template whose associated constraints are
// not satisfied is ill-formed; with dependent arguments it is checked at
// substitution, as a deduction failure (WP-309).
//@ expect: compile-fail
//@ strict-diags
// Verified against clang 19 (WP-309).
//@ diag: 15:1 error "constraints not satisfied for alias template 'A' [with T = char]"
//@ diag: 14:11 note "because 'char' does not satisfy 'Four'"
//@ diag: 13:35 note "because 'sizeof(char) == 4' (1 == 4) evaluated to false"
//@ diag: 19:12 error "no matching function for call to 'f'"
//@ diag: 16:25 note "candidate template ignored: substitution failure [with T = char]: constraints not satisfied for alias template 'A' [with T = char]"
template <class T> concept Four = sizeof(T) == 4;
template <Four T> using A = T;
A<char> x;
template <class T> void f(A<T>) {}
A<int> y;
template <class T> using D = A<T>; // dependent: not checked here
void g() { f<char>(0); }
