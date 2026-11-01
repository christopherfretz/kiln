// Vtables of class template specializations (WP-164, the libc++
// `__shared_ptr_pointer` link failure): an implicit instantiation has no
// key function (Itanium §5.2.6), so its vtable is linkonce_odr and uses
// every virtual member, including those defined out of line and after the
// point of instantiation; the vtable entries name the specialization's
// members. An explicit specialization keeps its key function (strong or
// external vtable); an explicitly specialized member keeps its slot. No vtable for the pattern. (Explicit instantiation
// definitions, `weak_odr` vtables: WP-113.) `typeid` of a template
// parameter. An inline virtual destructor in the base puts the
// specializations' implicit destructors (complete and deleting) in their
// vtables, emitted in clang's end-of-TU instantiation order (WP-172,
// spec/codegen.md §2a).
namespace std { class type_info; }

struct Count {
  virtual ~Count() {}
  virtual void on_zero() noexcept = 0;
  virtual const void *get_deleter(const std::type_info &) const noexcept;
};
const void *Count::get_deleter(const std::type_info &) const noexcept { return nullptr; }

struct Del {
  void operator()(int *p) const noexcept { *p = 0; }
};

template <class T, class D>
struct Pointer : Count {
  T p;
  D d;
  Pointer(T p, D d) noexcept : p(p), d(d) {}
  const void *get_deleter(const std::type_info &) const noexcept override;
  void on_zero() noexcept override;
};
template <class T, class D>
const void *Pointer<T, D>::get_deleter(const std::type_info &t) const noexcept {
  return &t == &typeid(D) ? &d : nullptr;
}
template <class T, class D>
void Pointer<T, D>::on_zero() noexcept { d(p); }

// Explicit specialization, key function defined elsewhere: external vtable.
template <>
struct Pointer<char, char> : Count {
  void on_zero() noexcept override;
};
Count *make_ext() { return new Pointer<char, char>; }

Count *make(int *p) { return new Pointer<int *, Del>(p, Del{}); }

// Explicit specialization, key function defined here: strong vtable.
template <>
struct Pointer<long, long> : Count {
  long v;
  Pointer(long v) noexcept : v(v) {}
  void on_zero() noexcept override;
};
void Pointer<long, long>::on_zero() noexcept { v = 0; }
Count *make_spec() { return new Pointer<long, long>(7); }

// A virtual member defined after the point of instantiation.
template <class T>
struct Late : Count {
  T v;
  Late(T v) noexcept : v(v) {}
  void on_zero() noexcept override;
};
Count *make_late() { return new Late<int>(9); }
template <class T>
void Late<T>::on_zero() noexcept { v = 0; }

// An explicitly specialized virtual member keeps its slot.
template <>
void Late<char>::on_zero() noexcept { v = 1; }
Count *make_late_char() { return new Late<char>('a'); }
