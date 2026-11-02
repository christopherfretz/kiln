// [basic.link]/8: an entity whose type as written (a function's return or
// parameter type, through typedefs), or whose template arguments, involve
// a type with internal linkage or none is not usable from another
// translation unit; clang gives it internal linkage (`getLVForNamespaceScopeDecl`,
// `getLVForClassMember`, `mergeTemplateLV`), also when it is inline.
// A deduced return type is not part of the type as written.
namespace { struct A { int x; }; }
A ret() { return A{1}; }
int param(A a) { return a.x; }
typedef A TA;
TA ret_td() { return TA{2}; }
A *ret_ptr() { return nullptr; }
inline A ret_inline() { return A{3}; }
auto ret_deduced() { return A{4}; }
auto ret_trailing() -> A { return A{5}; }
template <class T> T make() { return T{6}; }
template <class T> int none() { return 7; }
template <> int none<A>() { return 8; }
template <class T> struct X { int f() { return 9; } static int s; };
template <class T> int X<T>::s = 10;
static int gi;
template <int *P> int nt() { return *P; }
struct S { A get(); static A sget(); int use(A); };
A S::get() { return A{11}; }
A S::sget() { return A{12}; }
int S::use(A a) { return a.x; }
int local() {
  struct L { int y; };
  X<L> xl;
  return xl.f();
}
int main() {
  ret(); param(A{}); ret_td(); ret_ptr(); ret_inline(); ret_deduced();
  ret_trailing(); make<A>(); none<A>(); none<int>();
  X<A> x; x.f(); X<A *> y; y.f(); X<A>::s = 4;
  nt<&gi>();
  S s; s.get(); S::sget(); s.use(A{});
  return local();
}
