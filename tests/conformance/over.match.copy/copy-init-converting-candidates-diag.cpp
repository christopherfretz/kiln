// [over.match.copy]/1.1: copy-initialization from a non-class expression
// considers only converting constructors (clang isConvertingConstructor
// on the templated declaration, through inherited constructors); a
// constructor (template) needing two arguments is not a noted candidate.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:9 error "no viable conversion from 'int' to 'S<long>'"
//@ diag: 27:26 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const S<long> &' for 1st argument"
//@ diag: 27:26 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'S<long> &&' for 1st argument"
//@ diag: 27:30 note "candidate constructor not viable: no known conversion from 'int' to 'long *' for 1st argument"
//@ diag: 30:3 error "no viable conversion from 'int' to 'P'"
//@ diag: 29:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const P &' for 1st argument"
//@ diag: 29:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'P &&' for 1st argument"
//@ diag: 29:12 note "candidate constructor not viable: no known conversion from 'int' to 'int *' for 1st argument"
//@ diag: 29:88 note "candidate constructor not viable: no known conversion from 'int' to 'void *' for 1st argument"
//@ diag: 33:3 error "no viable conversion from 'int' to 'Q'"
//@ diag: 32:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const Q &' for 1st argument"
//@ diag: 32:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'Q &&' for 1st argument"
//@ diag: 32:28 note "candidate constructor not viable: no known conversion from 'int' to 'double *' for 1st argument"
//@ diag: 31:43 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 35:3 error "no viable conversion from 'int' to 'X'"
//@ diag: 34:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const X &' for 1st argument"
//@ diag: 34:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'X &&' for 1st argument"
//@ diag: 34:29 note "candidate constructor not viable: no known conversion from 'int' to 'char *' for 1st argument"
//@ diag: 34:21 note "explicit constructor is not a candidate"
//@ diag: 34:66 note "explicit constructor is not a candidate"
template<class T> struct S { S(T*); S(T, T); template<class U> S(U, U); explicit S(int, int, int); S(); };
S<long> s = 1;
struct P { P(int*); P(int, int); P(int, int, int = 0, ...); template<class T> P(T, T); P(void*, ...); };
P p = 1;
struct B { B(int, int); template<class T> B(T*, int = 0); template<class T> B(T, T); };
struct Q : B { using B::B; Q(double*); };
Q q = 1;
struct X { explicit X(int); X(char*); template<class T> explicit X(T, int = 0); };
X x = 1;
