// [temp.inst], Itanium C++ ABI §2.5: the vtable dump names the virtual members of class template
// specializations as clang's __PRETTY_FUNCTION__ does without the `virtual` prefix: the scope
// with its (canonical) arguments, the parameter and return types of the pattern as written, and
// the `[T = int, N = 3]` argument list (also in the thunk and index blocks).
//@ expect: vtables
namespace n { struct X {}; }
template<class T, int N> struct A { virtual void f(T); virtual T g() const; virtual ~A(); };
template<class T, int N> void A<T, N>::f(T) {}
template<class T, int N> T A<T, N>::g() const { return T(); }
template<class T, int N> A<T, N>::~A() {}
template struct A<int, 3>;
struct B : A<long, 2> { void f(long) override; };
void B::f(long) {}
template<class T> struct O {
  struct I { virtual void f(); virtual ~I(); };
  template<class U> struct J { virtual U g(T); };
};
template<class T> void O<T>::I::f() {}
template<class T> O<T>::I::~I() {}
template<class T> template<class U> U O<T>::J<U>::g(T) { return U(); }
template struct O<n::X>::I;
template struct O<int>::J<char>;
template<auto V> struct P { virtual decltype(V) h(); };
template<auto V> decltype(V) P<V>::h() { return V; }
template struct P<5u>;
struct L { virtual void f(); };
struct M : virtual L { void f() override; };
void L::f() {}
void M::f() {}
template<class T> struct Q : M { void f() override; };
template<class T> void Q<T>::f() {}
template struct Q<int>;
