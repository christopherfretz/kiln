// [temp.inst]/11, [basic.def.odr]/8, Itanium C++ ABI §5.2.6 (WP-164): the
// vtable of a class template specialization uses its virtual members, so
// those defined out of line (as libc++'s `__shared_ptr_pointer` defines
// `__on_zero_shared`, `__get_deleter`) are instantiated, and no vtable is
// emitted for the template pattern itself. A template instantiation has
// no key function; an explicit specialization keeps one. Also a member
// defined after the point of instantiation, an explicitly specialized
// virtual member (keeps its slot), `typeid` of a template parameter
// ([expr.typeid]) and the trivial copy of an empty base that shares its
// address with a pointer member ([intro.object]/8, the EBO of libc++'s
// `__compressed_pair`).
//@ expect: run
//@ stdout: zero 3 deleter 1 other 0
//@ stdout: spec 7 late 9
//@ stdout: late char x
//@ stdout: pair 42
extern "C" int printf(const char *, ...);
namespace std { class type_info; }

struct Count {
  virtual ~Count();
  virtual void on_zero() noexcept = 0;
  virtual const void *get_deleter(const std::type_info &) const noexcept;
};
Count::~Count() {}
const void *Count::get_deleter(const std::type_info &) const noexcept { return nullptr; }

struct Del {
  int *seen;
  void operator()(int *p) const { *seen = *p; }
};

template <class T, class D>
struct Pointer : Count {
  T p;
  D d;
  Pointer(T p, D d) : p(p), d(d) {}
  const void *get_deleter(const std::type_info &) const noexcept override;
  void on_zero() noexcept override;
};

template <class T, class D>
const void *Pointer<T, D>::get_deleter(const std::type_info &t) const noexcept {
  return &t == &typeid(D) ? &d : nullptr;
}
template <class T, class D>
void Pointer<T, D>::on_zero() noexcept { d(p); }

// An explicit specialization is an ordinary class: its key function
// `on_zero` is defined below, so its vtable is emitted there.
template <>
struct Pointer<long, long> : Count {
  long v;
  Pointer(long v) : v(v) {}
  void on_zero() noexcept override;
};
void Pointer<long, long>::on_zero() noexcept { printf("spec %ld ", v); }

// A virtual member defined after the point of instantiation.
template <class T>
struct Late : Count {
  T v;
  Late(T v) : v(v) {}
  void on_zero() noexcept override;
};
Count *make_late() { return new Late<int>(9); }
template <class T>
void Late<T>::on_zero() noexcept { printf("late %d\n", (int)v); }

// An explicitly specialized virtual member keeps its vtable slot.
template <>
void Late<char>::on_zero() noexcept { printf("late char %c\n", v); }

// The EBO of a compressed pair: the empty element's trivial copy writes
// nothing over the pointer it overlaps.
struct Empty {};
template <class E>
struct Elem : E {
  template <class U>
  Elem(U &&u) : E(static_cast<U &&>(u)) {}
};
struct PtrElem {
  int *p;
};
struct Pair : PtrElem, Elem<Empty> {
  Pair(int *p, Empty e) : PtrElem{p}, Elem<Empty>(e) {}
};

int main() {
  int seen = 0, three = 3;
  Count *c = new Pointer<int *, Del>(&three, Del{&seen});
  c->on_zero();
  printf("zero %d deleter %d other %d\n", seen, c->get_deleter(typeid(Del)) != nullptr,
         c->get_deleter(typeid(int)) != nullptr);
  delete c;
  Count *s = new Pointer<long, long>(7);
  s->on_zero();
  delete s;
  Count *l = make_late();
  l->on_zero();
  delete l;
  Count *lc = new Late<char>('x');
  lc->on_zero();
  delete lc;
  int v = 42;
  Pair pr(&v, Empty{});
  printf("pair %d\n", *pr.p);
}
