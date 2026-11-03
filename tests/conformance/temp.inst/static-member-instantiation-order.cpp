// [temp.inst]/4, /5: an implicitly instantiated static data member's
// definition, out-of-line or an inline member's in-class initializer, is
// instantiated at the end of the translation unit in the order the uses
// queued them (clang `DoMarkVarDeclReferenced`, `PerformPendingInstantiations`);
// a member usable in constant expressions is instantiated at its use.
// The errors appear in clang's order, each with the note chain of the
// use that queued it (WP-186).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:33 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 32:28 note "in instantiation of static data member 'A<int>::x' requested here"
//@ diag: 29:52 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 32:40 note "in instantiation of static data member 'B<int>::i' requested here"
//@ diag: 31:47 error "type 'char' cannot be used prior to '::' because it has no members"
//@ diag: 32:44 note "in instantiation of function template specialization 'tf<char>' requested here"
//@ diag: 28:33 error "type 'char' cannot be used prior to '::' because it has no members"
//@ diag: 31:43 note "in instantiation of static data member 'A<char>::x' requested here"
//@ diag: 32:44 note "in instantiation of function template specialization 'tf<char>' requested here"
//@ diag: 29:52 error "type 'char' cannot be used prior to '::' because it has no members"
//@ diag: 32:66 note "in instantiation of static data member 'B<char>::i' requested here"
//@ diag: 28:33 error "type 'long' cannot be used prior to '::' because it has no members"
//@ diag: 32:79 note "in instantiation of static data member 'A<long>::x' requested here"
//@ diag: 33:28 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 34:21 note "in instantiation of variable template specialization 'vt<int>' requested here"
//@ diag: 28:33 error "type 'short' cannot be used prior to '::' because it has no members"
//@ diag: 34:41 note "in instantiation of static data member 'A<short>::x' requested here"
template<class T> struct A { static int x; };
template<class T> int A<T>::x = T::bad_x;
template<class T> struct B { static inline int i = T::bad_i; };
template<class T> struct C { static constexpr int c = T::bad_c; };
template<class T> int tf() { return A<T>::x + T::bad_tf; }
int use() { return A<int>::x + B<int>::i + tf<char>() + B<char>::i + A<long>::x; }
template<class T> int vt = T::bad_vt;
int use2() { return vt<int> + A<short>::x; }
