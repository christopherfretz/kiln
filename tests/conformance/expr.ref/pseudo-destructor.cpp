// [expr.ref]/1, [expr.prim.id.dtor], [expr.call]/5: `p->~T()` calls the
// destructor of a class object; for a scalar it is a pseudo-destructor call
// of type void that ends the object's lifetime and has no other effect.
//@ expect: run
//@ stdout: dtor 1 | dtor 2 | 7 7 | 3
extern "C" int printf(const char*, ...);
void* operator new(unsigned long, void* p) noexcept { return p; }

struct Noisy {
  int id;
  ~Noisy() { printf("dtor %d | ", id); }
};
typedef int Int;

template <class T> constexpr void destroy(T* p) { p->~T(); }
template <class T> void destroy_ref(T& r) { r.~T(); }

union U {
  int i;
  float f;
};
constexpr int scalar_lifetime() {
  U u{3};
  int r = u.i;
  u.i.~Int();
  u.f = 1.0f;
  u.i = 4;
  destroy(&u.i);
  return r;
}
static_assert(scalar_lifetime() == 3);

int main() {
  alignas(Noisy) unsigned char buf[sizeof(Noisy)];
  Noisy* n = new (static_cast<void*>(buf)) Noisy{1};
  n->~Noisy();
  n = new (static_cast<void*>(buf)) Noisy{2};
  destroy_ref(*n);
  int v = 7;
  Int* pv = &v;
  pv->~Int();
  destroy(pv);
  destroy_ref(v);
  printf("%d %d | %d\n", v, *pv, scalar_lifetime());
}
