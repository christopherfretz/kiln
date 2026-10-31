// Covariant returns: a return-adjusting thunk (_ZTch) and a this+return
// one. (noexcept: no EH before WP-100.)
struct A { int a = 1; virtual A* self() noexcept { return this; } virtual ~A() noexcept {} };
struct X { long x = 9; virtual void fx() noexcept {} };
struct B : X, A {
  B* self() noexcept override { return this; }
};
struct R { virtual R& me() noexcept { return *this; } };
struct S : X, R { S& me() noexcept override { return *this; } };
int use(A* a) noexcept { return a->self()->a; }
int run() noexcept {
  B b;
  S s;
  R& r = s;
  r.me();
  return use(&b) + b.self()->a;
}
