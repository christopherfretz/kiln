// [class.derived.general]/2, [temp.inst]/2: a base-specifier naming a class
// template specialization whose instantiation fails there is dropped (clang's
// CheckBaseSpecifier RequireCompleteType), so names of that base are not
// found in the derived class; a using-declaration whose qualifier names the
// failed class is not diagnosed again ([namespace.udecl]/3); a base whose
// qualifier fails to instantiate is "expected class name" at the name after
// the qualifier.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:40 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 27:12 note "in instantiation of template class 'A<int>' requested here"
//@ diag: 27:32 error "unknown type name 'Ty'"
//@ diag: 27:29 error "use of undeclared identifier 'k'"
//@ diag: 27:49 error "use of undeclared identifier 'f'"
//@ diag: 26:40 error "type 'long' cannot be used prior to '::' because it has no members"
//@ diag: 28:18 note "in instantiation of template class 'A<long>' requested here"
//@ diag: 29:40 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 30:12 note "in instantiation of template class 'V<int>' requested here"
//@ diag: 30:31 error "only virtual member functions can be marked 'override'"
//@ diag: 26:40 error "type 'char' cannot be used prior to '::' because it has no members"
//@ diag: 31:31 note "in instantiation of template class 'A<char>' requested here"
//@ diag: 32:9 note "in instantiation of template class 'C<char>' requested here"
//@ diag: 33:40 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 34:12 note "in instantiation of template class 'P<int>' requested here"
//@ diag: 34:20 error "expected class name"
template <class T> struct A { typename T::type m; int k; void f(); typedef int Ty; };
struct D : A<int> { int z = k; Ty t; void g() { f(); } };
struct E { using A<long>::k; };
template <class T> struct V { typename T::type m; virtual void vf() {} };
struct W : V<int> { void vf() override; };
template <class T> struct C : A<T> { using A<T>::k; using A<T>::f; };
C<char> c;
template <class T> struct P { typename T::type m; struct In {}; };
struct Q : P<int>::In {};
