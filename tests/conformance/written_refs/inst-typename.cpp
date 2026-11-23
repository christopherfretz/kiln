// Per-specialization entries (spec/ast.md §18.9): names disambiguated by
// `typename` and `template` ([temp.res.general]/3, [temp.names]/4) are
// looked up at instantiation: a typename-specifier names the member type
// (a typedef, a class), `typename T::template rebind<U>::other` names the
// member template `rebind` and the member `other` of its specialization,
// `t.template get<0>()` and `T::template make<int>()` the member function
// templates' specializations.
//@ written-refs: inst-typename.refs
struct Alloc {
  using value_type = int;
  struct node {};
  template <class U> struct rebind {
    using other = U *;
  };
  template <int I> int get() const { return I; }
  template <class U> static U make() { return U(); }
};

template <class T> int use(const T &t) {
  typename T::value_type a = 0;
  typename T::node n;
  typename T::template rebind<long>::other p = nullptr;
  (void)n;
  (void)p;
  return a + t.template get<0>() + T::template make<int>();
}

int run() { return use(Alloc{}); }
