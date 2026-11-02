// [enum.udecl]/1-2, [temp.inst]: a block-scope using-enum-declaration in a
// templated function names a non-dependent enumeration; each
// instantiation redeclares its enumerators in the instantiated block
// (libstdc++ bits/unicode.h `_Grapheme_cluster_iterator::_M_is_break`,
// WP-323). Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags

namespace detail {
enum class gcb { control, cr, lf, other };
}

template<class It> struct grapheme_iter {
  It cur;
  constexpr bool is_break(detail::gcb p1, detail::gcb p2) const {
    using enum detail::gcb;
    if (p1 == control || p1 == lf)
      return true;
    if (p1 == cr)
      return p2 != lf;
    switch (p2) {
    case control:
    case cr:
      return true;
    default:
      return false;
    }
  }
};

template<class T> constexpr int count_breaks() {
  using enum detail::gcb;
  grapheme_iter<T> g{};
  return g.is_break(cr, lf) + g.is_break(lf, other) + g.is_break(other, cr);
}

static_assert(count_breaks<const char*>() == 2);
static_assert(!grapheme_iter<int>{}.is_break(detail::gcb::other, detail::gcb::lf));

// A generic lambda's body: instantiated with the lambda's call operator.
constexpr int via_lambda = [](auto x) {
  using enum detail::gcb;
  return int(x) + int(other);
}(1);
static_assert(via_lambda == 4);
