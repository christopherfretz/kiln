// Static data members initialized in the class: a use that is not an
// odr-use ([basic.def.odr]/5, the lvalue-to-rvalue conversion is applied
// to a constant) folds to the value and references no symbol, also in a
// default argument and inside a try block or handler (libc++'s
// `memory_resource::allocate(size_t, size_t = __max_align)`); an odr-use
// (reference binding, address-of, pass by const reference) of a constexpr
// member, an inline variable ([class.static.data]/4, [dcl.constexpr]/1),
// emits it as a linkonce_odr constant; one of a non-inline `const` member
// is available_externally, its definition elsewhere.
using size_t = decltype(sizeof 0);

struct Res {
  static const size_t max_align = alignof(long double);
  static constexpr int ce = 7;
  static constexpr long unused = 9;
  void* allocate(size_t bytes, size_t align = max_align) { return do_allocate(bytes, align); }
  virtual void* do_allocate(size_t, size_t);
};
Res* get() noexcept;

void in_try() {
  try {
    (void)get()->allocate(1);
  } catch (...) {
    (void)get()->allocate(Res::max_align);
  }
}

void take(const int&);
void take_long(const long&);
const int* addr_ce() { return &Res::ce; }
const int& ref_ce() { return Res::ce; }
void pass_ce() { take(Res::ce); }
long value_only() { return Res::ce + Res::unused + (long)Res::max_align; }

template <class T> struct Tmpl {
  static constexpr T v = T(3);
  static constexpr T w = T(4);
  static const T k = T(5);
};
const int* addr_tmpl() { return &Tmpl<int>::v; }
void pass_tmpl() { take_long(Tmpl<long>::v); }
const int* addr_tmpl_k() { return &Tmpl<int>::k; }
const size_t* addr_max_align() { return &Res::max_align; }
long value_tmpl() {
  try {
    return Tmpl<long>::w + Tmpl<long>::k;
  } catch (...) {
    return Tmpl<int>::w;
  }
}
