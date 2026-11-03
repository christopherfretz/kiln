// [temp.point]/7, [basic.stc.thread], Itanium ABI §3.4: a thread-local
// variable template specialization and a thread-local static data member
// of a class template specialization are defined at the end of the
// translation unit (WP-466), so their initializers, _ZTH aliases and
// _ZTW wrappers follow the TU's functions, in instantiation order.
int g();
template <class T> thread_local T tv = g();
template <class T> struct A { static thread_local T s; };
template <class T> thread_local T A<T>::s = g() + 1;
int f() { return tv<int> + A<int>::s; }
int h() { return 3; }
