// [temp.res.general]/3, [over.match.class.deduct]/1, [temp.inst]/3: the
// implicit deduction guide of a constructor names members of the class
// template pattern (`span<O, rank_>` with `rank_` a static data member,
// an enumerator, a member of a member class); substituting
// the deduced arguments into the guide finds them in the specialization
// named by the substituted injected-class-name. The libc++
// `layout_stride::mapping` constructors (`__mdspan/layout_stride.h`).
//@ expect: compile-ok
//@ strict-diags
template <class T, int N> struct span { constexpr span(T*) {} };

struct ext2 { static constexpr int rank = 2; };
struct ext3 { static constexpr int rank = 3; };
int* p;

// A member class template of a non-template class.
struct layout {
  template <class E> class mapping;
};
template <class E>
class layout::mapping {
  static constexpr int rank_ = E::rank;
public:
  template <class O>
  constexpr mapping(const E&, span<O, rank_>) {}
};
static_assert(__is_same(decltype(layout::mapping(ext2(), span<int, 2>(p))),
                        layout::mapping<ext2>));
static_assert(__is_same(decltype(layout::mapping(ext3(), span<char, 3>(nullptr))),
                        layout::mapping<ext3>));

// A class template; an enumerator and a static member.
template <class E> struct m2 {
  enum { r = E::rank };
  static constexpr int twice = 2 * E::rank;
  template <class O> constexpr m2(E, span<O, r>, span<O, twice>) {}
};
static_assert(__is_same(decltype(m2(ext3(), span<int, 3>(p), span<int, 6>(p))), m2<ext3>));

// A member of a member class of the pattern.
template <class E> struct m3 {
  struct inner { static constexpr int n = E::rank + 1; };
  template <class O> constexpr m3(E, span<O, inner::n>) {}
};
static_assert(__is_same(decltype(m3(ext2(), span<long, 3>(nullptr))), m3<ext2>));
