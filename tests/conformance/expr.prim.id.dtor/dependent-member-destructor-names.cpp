// [expr.prim.id.dtor]/2, [basic.lookup.qual]/5, [temp.dep.expr]/3: in a
// member access whose object type depends on a template parameter, a
// destructor name the definition context cannot resolve (`~Base` naming a
// class template, `~Alias` naming a member of the object's class), a
// `~decltype(e)` destructor name, and a dependent nested-name-specifier
// ending in the destructor's class (`p->Base<T>::~Base()`) are kept and
// resolved at instantiation in the object's class; the scalar
// pseudo-destructor forms are accepted, and in a constant expression the
// explicit destructor call ends the object's lifetime.
//@ expect: run
//@ strict-diags
//@ stdout: ~Base 1
//@ stdout: ~S 2
//@ stdout: ~S 3
//@ stdout: ~Base 4
//@ stdout: ~Base 5
//@ stdout: ~Base 6
//@ stdout: ~Base 7
extern "C" int printf(const char *, ...);
template <class T> struct Base { T v; ~Base() { printf("~Base %d\n", (int)v); } };
template <class T> struct Box : Base<T> {};
struct S { using Alias = S; int k; ~S() { printf("~S %d\n", k); } };
template <class T> void k6(Base<T> *b) { b->~Base(); }
template <class T> void k8(T *b) { b->~Alias(); }
template <class T> void k9(T *p) { T &b = *p; b.~decltype(T{})(); }
template <class T> void k10(T *b) { b->~decltype(T())(); }
template <class T> void q1(Base<T> *p) { p->Base<T>::~Base(); }
template <class T> void q2(Box<T> *p) { p->Box<T>::~Box(); }
template <class T> void q3(T *p) { p->Base<int>::~Base(); }
template <class T> void scalar(T x) { x.~T(); x.~decltype(x)(); }
struct C { int v; constexpr ~C() {} };
template <class T> constexpr int ce3() {
  union U { T t; constexpr U() : t{1} {} constexpr ~U() {} } u;
  int r = u.t.v;
  u.t.~T();
  return r;
}
static_assert(ce3<C>() == 1);
// Heap objects, so each is destroyed exactly once (by the call under test).
template <class T> T *make(int v) { T *p = new T; p->v = v; return p; }
S *mks(int k) { S *p = new S; p->k = k; return p; }
int main() {
  Base<int> *b1 = make<Base<int>>(1); k6(b1); ::operator delete(b1);
  S *s2 = mks(2); k8(s2); ::operator delete(s2);
  S *s3 = mks(3); k9(s3); ::operator delete(s3);
  Base<int> *b4 = make<Base<int>>(4); k10(b4); ::operator delete(b4);
  Base<int> *b5 = make<Base<int>>(5); q1(b5); ::operator delete(b5);
  Box<int> *x6 = make<Box<int>>(6); q2(x6); ::operator delete(x6);
  Base<int> *b7 = make<Base<int>>(7); q3(b7); ::operator delete(b7);
  scalar(1);
  return 0;
}
