// [temp.inst]/11, [temp.mem.func]/1: once a member function of a class
// template is instantiated from its out-of-line definition, the
// specialization is located at that definition (clang
// `InstantiateFunctionDefinition` copies the pattern definition's location):
// diagnostics in its body that point at the function, and later candidate
// and "declared here" notes, name the out-of-line definition, while a member
// whose definition was not instantiated is still noted at its in-class
// declaration.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:8 error "no matching constructor for initialization of 'C<int>'"
//@ diag: 23:3 note "candidate constructor not viable: requires 2 arguments, but 3 were provided"
//@ diag: 21:26 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 3 were provided"
//@ diag: 21:26 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 3 were provided"
//@ diag: 28:7 note "candidate constructor not viable: requires 0 arguments, but 3 were provided"
//@ diag: 34:24 error "too many arguments to function call, expected 0, have 1"
//@ diag: 30:12 note "'g' declared here"
//@ diag: 36:25 error "constructor for 'E<int>' must explicitly initialize the reference member 'r'"
//@ diag: 37:8 note "in instantiation of member function 'E<int>::E' requested here"
//@ diag: 35:38 note "declared here"
template<class T> struct C {
  C();
  C(int, int);
  void g();
  T t;
};
template<class T>
C<T>::C() {}
template<class T>
void C<T>::g() {}
template C<int>::C();
template void C<int>::g();
C<int> d(1, 2, 3);
void f(C<int> c) { c.g(1); }
template<class T> struct E { E(); T& r; };
template<class T> E<T>::E() {}
E<int> e;
