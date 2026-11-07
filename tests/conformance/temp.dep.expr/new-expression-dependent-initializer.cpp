// [temp.dep.expr]/3-4: a new-expression is type-dependent only if its
// new-type-id is; a type-dependent new-initializer, array bound or
// placement argument (`new D(I())`) makes it value-dependent, so its
// type `D *` is known in the template definition. A direct-
// initialization `P p(new D(I()))` therefore picks P's explicit
// constructor there, not a copy-initialization ("no viable conversion",
// libstdc++ `std::unique_ptr<B> u(new D(f()))` in a function template).
//@ expect: run
//@ strict-diags
#include <new>
struct B { virtual ~B() {} int v = 0; };
struct D : B { D(int x) { v = x; } };
struct P { explicit P(B *b) : p(b) {} ~P() { delete p; } B *p; };
template <class I> int direct() {
  P p(new D(I(3)));
  return p.p->v;
}
template <class I> int lambda_arg() {
  auto f = [] { return I(4); };
  P p(new D(f()));
  return p.p->v;
}
template <class I> int placement() {
  alignas(D) unsigned char buf[sizeof(D)];
  D *d = new (static_cast<void *>(buf)) D(I(5));
  int r = d->v;
  d->~D();
  return r;
}
template <class I> int deduced() {
  auto q = new D(I(6));
  static_assert(sizeof(q) == sizeof(D *));
  int r = q->v;
  delete q;
  return r;
}
int main() {
  return direct<int>() == 3 && lambda_arg<int>() == 4 && placement<int>() == 5 &&
                 deduced<int>() == 6
             ? 0
             : 1;
}
