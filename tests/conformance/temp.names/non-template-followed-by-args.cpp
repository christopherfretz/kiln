// [temp.names]/3: from C++20 a name lookup finds only functions for is
// assumed to be a template-name before `<`, but a variable, enumerator,
// parameter or data member never is. Such a single non-template followed
// by `<>` or by `<` type-id ... `>` is diagnosed as clang's
// checkPotentialAngleBracket does, in every language mode: "'v' does not
// name a template but is followed by template arguments" with a note at
// the declaration. A `<` followed by an expression stays a comparison.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 23:38 error "'x' does not name a template but is followed by template arguments"
//@ diag: 23:16 note "non-template declaration found by name lookup"
//@ diag: 24:9 error "'v' does not name a template but is followed by template arguments"
//@ diag: 21:5 note "non-template declaration found by name lookup"
//@ diag: 25:9 error "'e0' does not name a template but is followed by template arguments"
//@ diag: 22:10 note "non-template declaration found by name lookup"
//@ diag: 26:27 error "'p' does not name a template but is followed by template arguments"
//@ diag: 26:14 note "non-template declaration found by name lookup"
//@ diag: 26:40 error "'v' does not name a template but is followed by template arguments"
//@ diag: 21:5 note "non-template declaration found by name lookup"
int v;
enum E { e0 };
struct S { int x; int get() { return x<int>(0); } };
int a = v<int>(1);
int b = e0<>(2);
void use(int p) { int q = p<long>(3) + v<S>(0); }
template<class T> int t = 0;
int c = t<int> < 1;
