// [temp.arg.general]/9, [temp.arg.nontype]/2, [temp.dep.constexpr]/2: in a
// member of a class template, `sizeof(member)` is instantiation-dependent
// (through the implicit `this` of the current instantiation) but not
// value-dependent. A default argument after it is substituted with its
// value, so `AS<sizeof(buf)>` names `AS<32, 16>` and its partial
// specialization, not a primary specialization whose second argument is a
// dependent `mx<sizeof(buf)>::value` (libc++'s `aligned_storage` in
// `std::function`'s swap; a one-byte temporary buffer crashed it, WP-685).
//@ expect: run
//@ strict-diags

template <unsigned long N> struct mx { static const unsigned long value = 16; };
template <unsigned long L, unsigned long A = mx<L>::value>
struct AS {
  struct type { char d[1]; };
};
template <unsigned long L> struct AS<L, 16> {
  struct alignas(16) type { char d[L]; };
};

template <unsigned long L, unsigned long A = mx<L>::value>
struct P { static constexpr unsigned long a = A; };

struct B { char x[32]; };

template <class T> struct VF {
  B buf;
  static B sbuf;
  char raw[32];
  unsigned long member() { return sizeof(typename AS<sizeof(buf)>::type); }
  unsigned long qualified() { return sizeof(typename AS<sizeof(VF::buf)>::type); }
  unsigned long array() { return sizeof(typename AS<sizeof(raw)>::type); }
  unsigned long aligned() { return alignof(typename AS<sizeof(raw)>::type); }
  unsigned long value() { return P<sizeof(raw)>::a; }
  unsigned long static_member() { return sizeof(typename AS<sizeof(sbuf)>::type); }
  unsigned long local() { B l; return sizeof(typename AS<sizeof(l)>::type); }

  // The shape of libc++'s `__value_func::swap`: a temporary of the
  // buffer's size, filled from and copied back to it.
  void swap(VF& o) {
    typename AS<sizeof(raw)>::type tmp;
    for (unsigned i = 0; i < sizeof(raw); ++i) tmp.d[i] = raw[i];
    for (unsigned i = 0; i < sizeof(raw); ++i) raw[i] = o.raw[i];
    for (unsigned i = 0; i < sizeof(raw); ++i) o.raw[i] = tmp.d[i];
  }
};

int main() {
  VF<int> v, w;
  if (v.member() != 32 || v.qualified() != 32 || v.array() != 32) return 1;
  if (v.aligned() != 16 || v.value() != 16) return 2;
  if (v.static_member() != 32 || v.local() != 32) return 3;
  for (unsigned i = 0; i < 32; ++i) v.raw[i] = char(i), w.raw[i] = char(100 + i);
  v.swap(w);
  for (unsigned i = 0; i < 32; ++i)
    if (v.raw[i] != char(100 + i) || w.raw[i] != char(i)) return 4;
  return 0;
}
