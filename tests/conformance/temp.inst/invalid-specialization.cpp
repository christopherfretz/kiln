// [temp.inst]/2: a class template specialization whose instantiation
// fails (here a dependent base) is invalid; later uses of it are quiet.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:30 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 14:9 note "in instantiation of template class 'A<int>' requested here"
//@ diag: 15:30 error "type 'long' cannot be used prior to '::' because it has no members"
//@ diag: 17:20 note "in instantiation of template class 'B<long>' requested here"
//@ diag: 18:30 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 19:21 note "in instantiation of template class 'C<int>' requested here"
//@ diag: 20:30 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 21:12 note "in instantiation of template class 'D<int>' requested here"
template<class T> struct A : T::base { int x; };
int s = sizeof(A<int>);
template<class T> struct B : T::base { int x; };
void f(B<int>);
void g() { B<long> b; }
template<class T> struct C : T::base { int x; };
C<int> *p; int t = p->x;
template<class T> struct D : T::base { int x; };
struct E : D<int> {};
