// [temp.inst]/3, [temp.res.general]/1, [class.local]: a block-scope
// declaration of a function template specialization whose instantiation
// fails is diagnosed once; later uses of it (a variable, typedef or
// enumeration of the failed statement, the rest of its declaration
// statement, a local class made invalid by a member) are invalid without
// another diagnostic (clang keeps the invalid declaration in scope;
// WP-316: kiln said "not yet implemented: reference to a member of a
// dependent context"). A local class's body and member functions are
// instantiated under "in instantiation of member class" / "member
// function" notes at the outermost local class, named through the
// function when nested (clang InstantiateClass, InstantiateClassMembers).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 28:22 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 36:9 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 40:14 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 39:10 note "in instantiation of member class 'L' requested here"
//@ diag: 49:9 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 43:25 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 43:12 note "in instantiation of member class 'f()::L::N' requested here"
//@ diag: 39:10 note "in instantiation of member class 'L' requested here"
//@ diag: 49:9 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 42:26 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 39:10 note "in instantiation of member function 'f()::L::m' requested here"
//@ diag: 49:9 note "in instantiation of function template specialization 'f<int>' requested here"
template <class T> int g() {
  using A = typename T::type;
  A a, b = 1;
  enum E : A { e1 };
  typedef A B;
  B c = a + b;
  E e = e1;
  return sizeof(A) + a + b + c + e + sizeof(E);
}
int x = g<int>();

template <class T> int f() {
  struct L {
    typename T::type x;
    int y;
    int m() { return y + T::nope; }
    struct N { typename T::type z; };
  };
  L l{};
  struct P { int k() { return 1; } };
  return l.x + l.y + l.m() + sizeof(L::x) + L().y + P().k() + sizeof(typename L::N);
}
int y = f<int>();
