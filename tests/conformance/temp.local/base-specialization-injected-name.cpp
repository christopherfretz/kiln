// [temp.local]/1, [class.qual]/2, [class.member.lookup]: the
// injected-class-name of a base class template specialization, found by
// lookup in a derived class (unqualified inside it, or qualified `D::V`
// through a derived class or a typedef of one) and not followed by `<`,
// is a type-name naming that specialization, not the class template
// (no "requires template arguments"); followed by `<` it is the
// template. Also through a dependent base after instantiation
// (`typename C::V`).
//@ expect: run
//@ strict-diags
template <class E, class A = int> struct V { int x = sizeof(E); static int s; using type = E; };
template <class E, class A> int V<E, A>::s = 7;
template <class E, class A = int> struct D : V<E, A> {};
struct B : V<long> {
  V v;
  V<char> c;
  int f() { return V::s + V().x; }
};
template <class T> struct C : V<T> { typename C::V w; };
typedef D<char> Doc;
typedef Doc::V GV;
using GV2 = D<short>::V;
B::V bv;
B::V::type t = 1;
static_assert(sizeof(B::V::type) == sizeof(long));
static_assert(sizeof(Doc::V::type) == 1);
static_assert(sizeof(GV2::type) == sizeof(short));
int main() {
  GV g;
  B b;
  C<int> c;
  return (g.x == 1 && bv.x == (int)sizeof(long) && b.v.x == (int)sizeof(long) && b.c.x == 1 &&
          b.f() == 7 + (int)sizeof(long) && Doc::V().x == 1 && c.w.x == (int)sizeof(int) && t == 1)
             ? 0
             : 1;
}
