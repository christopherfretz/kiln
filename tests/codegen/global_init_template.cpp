// [basic.start.dynamic]/1: the initialization of a static data member of
// a class template specialization is unordered, and of an inline
// variable partially ordered: each gets its own llvm.global_ctors entry
// keyed to the variable's comdat (and llvm.used), and runs under a
// _ZGV guard -- acquired with __cxa_guard_acquire for an inline variable.
int f();
struct A { A(int); ~A(); int x; };
template <class T> struct Tm { static A m; static int n; static int k; };
template <class T> A Tm<T>::m(5);
template <class T> int Tm<T>::n = f();
template <class T> int Tm<T>::k = 3;
inline A ia(6);
inline int ii = f() + 1;
struct C { static inline A cm{7}; };
int g = f();
int use() { return Tm<int>::m.x + Tm<char>::n + Tm<int>::k + ia.x + ii + C::cm.x; }
