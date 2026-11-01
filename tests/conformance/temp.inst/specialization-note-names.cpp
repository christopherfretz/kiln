// [temp.inst]: an instantiation note names a specialization as clang's
// getNameForDiagnostic does: canonical arguments with every type scoped
// (never the typedef they were written with, e.g. `_AllocT`), and an
// inline namespace dropped when it is redundant for the next name.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:41 error "no member named 'nope' in 'std::D<int, int>'"
//@ diag: 44:15 note "in instantiation of variable template specialization 'std::v<std::D<int, int>>' requested here"
//@ diag: 24:29 error "no member named 'nope' in 'std::alloc<int>'"
//@ diag: 39:36 note "in instantiation of member function 'std::Ptr<int *, std::D<int, int>, std::alloc<int>>::g' requested here"
//@ diag: 25:31 error "no member named 'nope' in 'std::D<int, int>'"
//@ diag: 40:36 note "in instantiation of member function 'std::Ptr<long *, std::D<int, int>, std::alloc<int>>::s' requested here"
//@ diag: 33:23 error "no member named 'nope' in 'std::alloc<int>'"
//@ diag: 41:17 note "in instantiation of member function 'Q<std::alloc<int>>::h' requested here"
//@ diag: 28:35 error "no member named 'nope' in 'std::alloc<int>'"
//@ diag: 42:8 note "in instantiation of function template specialization 'std::f<std::alloc<int>>' requested here"
//@ diag: 35:34 error "no member named 'nope' in 'std::D<int, int>'"
//@ diag: 43:3 note "in instantiation of function template specialization 'g<std::D<int, int>>' requested here"
namespace std {
inline namespace __1 {
template <class T> struct alloc {};
template <class T, class U> struct D {};
template <class P, class Dp, class A> struct Ptr {
  int g() const { return A::nope; }
  static int s() { return Dp::nope; }
};
template <class T> constexpr int v = T::nope;
template <class T> void f(T) { T::nope; }
}
}
using IntAlloc = std::alloc<int>;
template <class X> struct Q {
  int h() { return X::nope; }
};
template <class T> void g() { T::nope; }
int main() {
  typedef std::alloc<int> _AllocT;
  typedef std::D<int, int> _Del;
  std::Ptr<int *, _Del, _AllocT>().g();
  std::Ptr<long *, _Del, _AllocT>::s();
  Q<IntAlloc>().h();
  std::f(_AllocT());
  g<_Del>();
  return std::v<_Del>;
}
