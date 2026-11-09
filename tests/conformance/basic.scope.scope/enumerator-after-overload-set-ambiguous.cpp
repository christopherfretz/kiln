// [basic.lookup.general]/4, [dcl.enum]/11: after an unscoped enumerator
// declared beside an overload set of its name, a lookup that finds both
// is ambiguous: unqualified, qualified into the enclosing class, member
// access, in a member function body, and inside the enumeration itself.
// A single earlier function is still a redefinition, as is a function
// after the enumerator (a different kind of entity). clang lists the
// candidates latest first from the scope chain and in declaration order
// from a class's lookup table, and names an unscoped enumerator without
// its enumeration (WP-929).
//@ expect: compile-fail
//@ strict-diags
// The block-scope candidates are checked without their names: clang
// prints the enumerator as 'k()::q' and the functions as 'q', kiln the
// other way round (wording follow-up).
//@ diag: 49:18 error "reference to 'f' is ambiguous"
//@ diag: 48:10 note "candidate found by name lookup is 'f'"
//@ diag: 47:5 note "candidate found by name lookup is 'f'"
//@ diag: 46:5 note "candidate found by name lookup is 'f'"
//@ diag: 50:12 error "reference to 'f' is ambiguous"
//@ diag: 48:10 note "candidate found by name lookup is 'f'"
//@ diag: 47:5 note "candidate found by name lookup is 'f'"
//@ diag: 46:5 note "candidate found by name lookup is 'f'"
//@ diag: 53:9 error "reference to 't' is ambiguous"
//@ diag: 52:10 note "candidate found by name lookup is 't'"
//@ diag: 51:24 note "candidate found by name lookup is 't'"
//@ diag: 54:67 error "reference to 'm' is ambiguous"
//@ diag: 54:40 note "candidate found by name lookup is 'S::m'"
//@ diag: 54:25 note "candidate found by name lookup is 'S::m'"
//@ diag: 54:16 note "candidate found by name lookup is 'S::m'"
//@ diag: 55:12 error "reference to 'm' is ambiguous"
//@ diag: 54:16 note "candidate found by name lookup is 'S::m'"
//@ diag: 54:25 note "candidate found by name lookup is 'S::m'"
//@ diag: 54:40 note "candidate found by name lookup is 'S::m'"
//@ diag: 56:23 error "reference to 'm' is ambiguous"
//@ diag: 54:16 note "candidate found by name lookup is 'S::m'"
//@ diag: 54:25 note "candidate found by name lookup is 'S::m'"
//@ diag: 54:40 note "candidate found by name lookup is 'S::m'"
//@ diag: 58:10 error "redefinition of 'one'"
//@ diag: 57:6 note "previous definition is here"
//@ diag: 60:6 error "redefinition of 'two' as different kind of symbol"
//@ diag: 59:10 note "previous definition is here"
//@ diag: 61:53 error "reference to 'q' is ambiguous"
//@ diag: 61:42 note "candidate found by name lookup is"
//@ diag: 61:27 note "candidate found by name lookup is"
//@ diag: 61:17 note "candidate found by name lookup is"
int f(int);
int f(double);
enum E { f = 7 };
int g() { return f; }
void h() { f(1); }
template <class T> int t(T);
enum F { t = 1 };
int u = t;
struct S { int m(); int m(int); enum { m = 2 }; int in() { return m; } };
int v = S::m;
int w(S s) { return s.m; }
void one();
enum G { one };
enum H { two };
void two();
void k() { void q(); void q(int); enum { q = 3, r = q }; }
