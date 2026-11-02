// [expr.ref]/4, [class.dtor]/16, [temp.res]/9: a non-dependent destructor
// call `p->~C()` / `p->B::~B()` / `x.~C()` inside a template is resolved at
// the definition and formed again at instantiation; access is checked in
// the instantiated member (libc++ allocate_shared.lwg2070's
// `FactoryAllocator<Private>::destroy` calling a private destructor of
// which it is a friend). A qualified call is not virtual.
//@ expect: run
//@ strict-diags
inline void* operator new(__SIZE_TYPE__, void* p) noexcept { return p; }
int log = 0;
template <class T> struct FA;
struct P {
  int id;
private:
  friend FA<P>;
  ~P() { log += 1; }
};
struct B { virtual ~B() { log += 10; } };
struct D : B { ~D() override { log += 100; } };
template <class T> struct FA {
  void destroy(P* p) { p->~P(); }
  void base_only(B* b) { b->B::~B(); }
  void any(B* b) { b->~B(); }
  void dot(D& d) { d.~D(); }
};
int main() {
  alignas(D) unsigned char buf[sizeof(D)];
  alignas(P) unsigned char pb[sizeof(P)];
  FA<P> f;
  f.destroy(new (pb) P);        // ~P: 1
  f.any(new (buf) D);           // virtual: ~D then ~B: 110
  f.base_only(new (buf) D);     // qualified: ~B only: 10
  f.dot(*new (buf) D);          // ~D then ~B: 110
  return log == 1 + 110 + 10 + 110 ? 0 : 1;
}
