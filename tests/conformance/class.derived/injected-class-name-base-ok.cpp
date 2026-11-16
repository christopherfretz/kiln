// [class.derived.general]/2, [temp.local]/1: an injected-class-name is a
// valid base: through the class (D::D), a typedef, a dependent base
// (T::D, TD<T>::TD), and in an out-of-line nested class or member
// template of a class template, whose enclosing class is complete.
//@ std: c++20
//@ expect: run
struct D { int d = 1; };
struct E : D::D { };
typedef D DT;
struct G : DT::D { };
template<class T> struct TD { int t = 2; };
template<class T> struct TF : TD<T> { struct M : TD<T>::TD { }; };
template<class T> struct H : T::D { };
template<class T> struct A { int foo = 3; struct N; template<int> struct B; };
template<class T> struct A<T>::N : A { using A::foo; };
template<class T> template<int K> struct A<T>::B : A { int k = K; };
int main() {
  E e;
  G g;
  TF<int>::M m;
  H<D> h;
  A<int>::N n;
  A<int>::B<4> b;
  return e.d + g.d + m.t + h.d + n.foo + b.foo + b.k == 15 ? 0 : 1;
}
