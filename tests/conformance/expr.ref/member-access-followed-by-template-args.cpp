// [expr.ref], [temp.names]/3: a class member access naming one non-template
// member (a member function, data member or static member, through '.',
// '->' or a qualified name, also of the current instantiation) followed
// by '<>' or '<' type-id ... '>' is "'g' does not name a template but is
// followed by template arguments" with a note at the member; the
// argument list is skipped and the access is an error. An overloaded
// member gets no such diagnosis, nor does a list that need not be
// template arguments; a bound member function as an operand of a binary
// operator is "reference to non-static member function must be called"
// and nothing more.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 51:21 error "'g' does not name a template but is followed by template arguments"
//@ diag: 49:16 note "non-template declaration found by name lookup"
//@ diag: 52:23 error "'g' does not name a template but is followed by template arguments"
//@ diag: 49:16 note "non-template declaration found by name lookup"
//@ diag: 53:21 error "'h' does not name a template but is followed by template arguments"
//@ diag: 49:63 note "non-template declaration found by name lookup"
//@ diag: 54:21 error "'s' does not name a template but is followed by template arguments"
//@ diag: 49:92 note "non-template declaration found by name lookup"
//@ diag: 55:21 error "'sv' does not name a template but is followed by template arguments"
//@ diag: 49:77 note "non-template declaration found by name lookup"
//@ diag: 56:21 error "'g' does not name a template but is followed by template arguments"
//@ diag: 49:16 note "non-template declaration found by name lookup"
//@ diag: 57:23 error "'g' does not name a template but is followed by template arguments"
//@ diag: 49:16 note "non-template declaration found by name lookup"
//@ diag: 58:24 error "'g' does not name a template but is followed by template arguments"
//@ diag: 49:16 note "non-template declaration found by name lookup"
//@ diag: 59:22 error "'g' does not name a template but is followed by template arguments"
//@ diag: 49:16 note "non-template declaration found by name lookup"
//@ diag: 60:21 error "'g' does not name a template but is followed by template arguments"
//@ diag: 49:16 note "non-template declaration found by name lookup"
//@ diag: 61:26 error "expected '(' for function-style cast or type construction"
//@ diag: 62:21 error "no member named 'zz' in 'A'"
//@ diag: 62:27 error "expected '(' for function-style cast or type construction"
//@ diag: 63:21 error "reference to non-static member function must be called"
//@ diag: 64:21 error "reference to non-static member function must be called"
//@ diag: 65:22 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 66:22 error "reference to non-static member function must be called"
//@ diag: 67:44 error "'g' does not name a template but is followed by template arguments"
//@ diag: 49:16 note "non-template declaration found by name lookup"
//@ diag: 68:47 error "'f' does not name a template but is followed by template arguments"
//@ diag: 68:16 note "non-template declaration found by name lookup"
//@ diag: 69:72 error "'f' does not name a template but is followed by template arguments"
//@ diag: 69:34 note "non-template declaration found by name lookup"
//@ diag: 69:109 error "'v' does not name a template but is followed by template arguments"
//@ diag: 69:46 note "non-template declaration found by name lookup"
struct A { int g(int); int g0(); int o(int); int o(char); int h; static int sv; static int s(int); };
A a; A *pa;
int n1() { return a.g<int>(1); }
int n2() { return pa->g<int>(1); }
int n3() { return a.h<int>(1); }
int n4() { return a.s<int>(1); }
int n5() { return a.sv<int>(1); }
int n6() { return a.g<>(1); }
int n7() { return pa->g<int, char>(1) + 2; }
int n8() { return a.A::g<int>(1); }
int n9() { return (a.g<int>)(1); }
int m1() { return a.g<int>; }
int m2() { return a.o<int>(1); }
int m3() { return a.zz<int>(1); }
int m4() { return a.g<1>(2); }
int m5() { return a.g + 1; }
bool m6() { return a.g0 < 1; }
int m7() { return (a.g < 1) > (2); }
template<class T> int t1() { A b; return b.g<int>(1); }
struct E { int f(int); int k() { return this->f<int>(1); } };
template<class T> struct D { int f(int); int v; int k() { return this->f<int>(1); } int k2() { return this->v<int>; } };
