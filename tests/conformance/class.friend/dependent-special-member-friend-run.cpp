// [class.friend]/4, [temp.friend]/1: a class template befriending a
// constructor, the destructor or a conversion function of a dependent class
// (`friend Z<T>::Z(typename T::t);`, `friend Z<T>::~Z();`,
// `friend Z<T>::operator int();`) befriends, in each specialization, the
// member of the class the substituted qualifier names whose parameter-type-list
// matches. kiln befriended nothing (found running GCC's C++ testsuite). Also:
// an assignment operator, a constructor template's specialization the type
// deduces ([temp.friend]/1.2), a non-dependent class's constructors with a
// dependent and a non-dependent parameter type, and a member class of the
// current instantiation (`friend Y2<T>::M::M(T);`, `friend M::~M();`). The
// -denied sibling shows the members it leaves out.
//@ skip(clang): clang befriends no constructor or destructor of a dependent class
//@ expect: run
//@ stdout: 1 2 5 4 6 7 8 9 10 3 3 3 end
extern "C" int printf(const char *, ...);
template <class T> class Y;
template <class T> struct Z {
  Z(typename T::t);
  Z();
  ~Z();
  operator int();
  Z &operator=(const Z &);
  template <class U> Z(U *);
};
struct P { typedef int t; };
struct S { S(); S(int); };
template <class T> class Y {
  friend Z<T>::Z(typename T::t);
  friend Z<T>::Z();
  friend Z<T>::~Z();
  friend Z<T>::operator int();
  friend Z<T> &Z<T>::operator=(const Z<T> &);
  friend Z<T>::Z(char *);
  friend S::S(typename T::t);
  friend S::S();
  static int x;
};
template <class T> struct Y2 {
  struct M { M(T); ~M(); };
  class G { friend Y2<T>::M::M(T); friend M::~M(); static int x; };
};
template <class T> int Y<T>::x = 1;
template <class T> Z<T>::Z(typename T::t) { printf("%d ", Y<T>::x); }
template <class T> Z<T>::Z() { printf("%d ", Y<T>::x + 1); }
template <class T> Z<T>::~Z() { printf("%d ", Y<T>::x + 2); }
template <class T> Z<T>::operator int() { return Y<T>::x + 3; }
template <class T> Z<T> &Z<T>::operator=(const Z &) { printf("%d ", Y<T>::x + 4); return *this; }
template <class T> template <class U> Z<T>::Z(U *) { printf("%d ", Y<T>::x + 5); }
S::S(int) { printf("%d ", Y<P>::x + 6); }
S::S() { printf("%d ", Y<P>::x + 7); }
template <class T> int Y2<T>::G::x = 9;
template <class T> Y2<T>::M::M(T) { printf("%d ", G::x); }
template <class T> Y2<T>::M::~M() { printf("%d ", G::x + 1); }
int main() {
  {
    Z<P> a(1), b;
    b = a;
    printf("%d ", int(a));
    char c;
    Z<P> d(&c);
    S s(1), s2;
    Y2<long>::M m(1L);
  }
  printf("end\n");
}
