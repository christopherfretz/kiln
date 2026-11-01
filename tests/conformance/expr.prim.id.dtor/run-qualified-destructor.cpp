// [expr.prim.id.dtor], [expr.ref]/4, [class.virtual]/16: a qualified
// destructor name calls the named destructor non-virtually, also through
// a base pointer and when substituted (`t->T::~T()`, `this->B::~B()`); a
// qualified pseudo-destructor on a scalar ends the object's lifetime
// ([basic.life]) and is a constant expression; `u.w.W::~W()` on a union
// member in a constexpr function ([expr.const]).
//@ expect: run
//@ stdout: ~B
//@ stdout: ~B
//@ stdout: ~D
//@ stdout: ~B
//@ stdout: ~B
//@ stdout: ~B
//@ stdout: ~B
//@ stdout: 7 7
extern "C" int puts(const char*);
extern "C" int printf(const char*, ...);
inline void* operator new(decltype(sizeof 0), void* p) noexcept { return p; }
struct B { virtual ~B() { puts("~B"); } };
struct D : B { ~D() override { puts("~D"); } };
struct W { int v; constexpr ~W() {} };
union U { W w; constexpr U() : w{7} {} constexpr ~U() {} };
constexpr int f() { U u; int r = u.w.v; u.w.W::~W(); return r; }
static_assert(f() == 7);
typedef int I;
union V { int i; int j; constexpr V() : i(3) {} constexpr ~V() {} };
constexpr int g() {
  V v;
  int r = v.i;
  (&v.i)->I::~I();
  v.j = 4;
  r += v.j;
  (&v.j)->::I::~I();
  return r;
}
static_assert(g() == 7);
template <class T> void kill(T* t) { t->T::~T(); }
template <class T> struct Holder {
  alignas(T) unsigned char buf[sizeof(T)];
  T* get() { return reinterpret_cast<T*>(buf); }
  void destroy() { get()->T::~T(); }
};
struct E : B { ~E() override { puts("~E"); } void self() { this->B::~B(); } };
int main() {
  alignas(D) unsigned char buf[sizeof(D)];
  D* d = new (buf) D;
  d->B::~B();
  B* b = new (buf) D;
  b->B::~B();
  b = new (buf) D;
  b->~B();
  Holder<B> h;
  new (h.get()) D;
  h.destroy();
  B* hb = new (h.get()) D;
  kill(hb);
  int i = 0;
  kill(&i);
  E* e = new (buf) E;
  e->self();
  printf("%d %d\n", f(), g());
}
