// [namespace.memdef]/3: `friend class X;` looks in the innermost enclosing
// namespace only, where a using-declaration naming a class of a nested
// namespace is found, and the friend is that class (boost's
// `friend class iterator_core_access;` in token_iterator, with
// `using iterators::iterator_core_access;` in namespace boost).
//@ strict-diags
namespace boost {
namespace iterators {
class core {
public:
  template <class F> static bool eq(const F &f) { return f.equal(); }
};
} // namespace iterators
using iterators::core;
class tok {
  friend class core;
  bool equal() const { return true; }
};
} // namespace boost
bool x = boost::core::eq(boost::tok());

// Also from a class template derived from a class of the nested namespace
// (boost::token_iterator over boost::iterators::iterator_facade).
namespace boost {
namespace iterators {
template <class D> struct facade {
  friend class core;
  bool ne(const D &o) const { return !core::eq(o); }
};
} // namespace iterators
template <class T> class it : public iterators::facade<it<T>> {
  friend class core;
  bool equal() const { return true; }
};
} // namespace boost
bool y = boost::it<int>().ne(boost::it<int>());
