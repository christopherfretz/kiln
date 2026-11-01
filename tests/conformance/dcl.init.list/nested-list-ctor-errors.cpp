// [dcl.init.list]/3.7, [over.match.list]: a nested braced-init-list
// element with no conversion to any constructor parameter.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:3 error "no matching constructor for initialization of 'G'"
//@ diag: 22:8 note "candidate constructor (the implicit copy constructor) not viable: cannot convert initializer list argument to 'const G'"
//@ diag: 22:8 note "candidate constructor (the implicit move constructor) not viable: cannot convert initializer list argument to 'G'"
//@ diag: 22:12 note "candidate constructor not viable: cannot convert initializer list argument to 'P'"
//@ diag: 25:7 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 25:7 note "insert an explicit cast to silence this issue"
//@ diag: 26:3 error "no matching constructor for initialization of 'H'"
//@ diag: 23:8 note "candidate constructor (the implicit copy constructor) not viable: cannot convert initializer list argument to 'const H'"
//@ diag: 23:8 note "candidate constructor (the implicit move constructor) not viable: cannot convert initializer list argument to 'H'"
//@ diag: 23:12 note "candidate constructor not viable: cannot convert initializer list argument to 'Q'"
//@ diag: 28:8 error "no matching constructor for initialization of 'E<int>'"
//@ diag: 27:30 note "candidate constructor not viable: requires 1 argument, but 2 were provided"
//@ diag: 27:26 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 27:26 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 2 were provided"
struct P { int a, b; };
struct Q { explicit Q(int, int); };
double d = 1.5;
struct G { G(P); };
struct H { H(Q); };
G g1{{1, 2, 3}};
G g2{{d, 2}};
H h1{{1, 2}};
template<class T> struct E { E(P); };
E<int> e{{1, 2}, 3};
