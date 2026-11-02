// [dcl.init.aggr]/1, [class.default.ctor]/1, [over.match.ctor]: a constructor
// (or constructor template) with a parameter that names no type is an
// invalid declaration. As clang's `addedMember` ignores it, it is not a
// user-declared constructor: the class stays an aggregate and gets its
// implicit default constructor, so a braced or parenthesized initializer is
// aggregate initialization ("excess elements in struct initializer"). It is
// no candidate in constructor overload resolution either, and not listed in
// "no matching constructor". In a class template, [temp.inst]: the invalid
// member of the pattern makes each specialization invalid (clang's
// `InstantiateClass`), whose initializations are not checked.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:27 error "unknown type name 'undeclared_t'"
//@ diag: 30:6 error "excess elements in struct initializer"
//@ diag: 35:28 error "unknown type name 'bad_t'"
//@ diag: 37:10 error "excess elements in struct initializer"
//@ diag: 39:4 error "excess elements in struct initializer"
//@ diag: 40:14 error "unknown type name 'undeclared_t'"
//@ diag: 41:6 error "excess elements in struct initializer"
//@ diag: 42:3 error "excess elements in struct initializer"
//@ diag: 45:27 error "unknown type name 'bad_t'"
//@ diag: 48:3 error "no matching constructor for initialization of 'D'"
//@ diag: 46:3 note "candidate constructor not viable: requires 1 argument, but 2 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 52:8 error "unknown type name 'bad_t'"
struct S {
  template <class T> S(T, undeclared_t);
};
S s1{1, 2};
S s2{};
S s3;
struct S2 {
  int a;
  template <class T> S2(T, bad_t);
};
S2 t1{1, 2};
S2 t2{1};
S2 t3(1, 2);
struct A { A(undeclared_t); };
A a1{1};
A a2(1);
static_assert(__is_aggregate(S) && __is_aggregate(A));
struct D {
  template <class T> D(T, bad_t);
  D(int);
};
D d1{1, 2};
D d2{1};
template <class U> struct C {
  U u;
  C(U, bad_t);
};
C<int> c1{1, 2};
C<int> c2{3};
