// [temp.spec.partial.member]/1, [temp.over.link]/6: an out-of-line member
// names the partial specialization whose template-head is equivalent to
// its own; with a requires-clause equivalent to no partial specialization
// with those arguments, the qualifier names none. Inside the out-of-line
// member of the constrained B<T>, `B<T>` is that partial specialization, so
// the primary's member I5 is not found. clang 22 wording.
// Found running GCC's C++ testsuite. WP-1509.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 18:33 error "nested name specifier 'A<T *>' for declaration does not refer into a class, class template or class template partial specialization"
//@ diag: 23:43 error "no type named 'I5' in 'B<T>'"
template <class T> concept is_int = __is_same_as(T, int);
template <class T> concept is_long = __is_same_as(T, long);
template <class T> struct A {};
template <is_int T> struct A<T*> { static int f(); };
template <class T> struct A<T*> { static int g(); };
template <is_long T> int A<T*>::f() { return 0; }

template <class T> struct B;
template <is_int T> struct B<T> { typedef int I4; static int f(); };
template <class T> struct B { typedef int I5; static int f(); };
template <is_int T> int B<T>::f() { B<T>::I5 i = 0; return i; }
