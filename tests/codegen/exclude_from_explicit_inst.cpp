// clang's `exclude_from_explicit_instantiation` (libc++'s
// `_LIBCPP_HIDE_FROM_ABI`): a member so marked is not covered by an
// explicit instantiation of its class ([temp.explicit]/10, /14), so under
// `extern template` it is implicitly instantiated and emitted
// linkonce_odr when used, and an explicit instantiation definition does
// not emit it (clang `InstantiateClassMembers`). Unmarked inline members
// of an `extern template` class are only declared at -O0.
#define EXCLUDE __attribute__((__exclude_from_explicit_instantiation__))

template <class T> struct B {
  EXCLUDE B() {}
  EXCLUDE int good() const { return 1; }
  [[clang::exclude_from_explicit_instantiation]] int good2() const { return 5; }
  EXCLUDE int later() const;
  int plain() const { return 2; }
  int out() const;
  struct EXCLUDE Nested {
    int get() const { return 6; }
  };
  struct Inner {
    int get() const { return 7; }
  };
};
template <class T> int B<T>::out() const { return 4; }
template <class T> int B<T>::later() const { return 8; }

extern template struct B<char>;
int use_char() {
  B<char> b;
  return b.good() + b.good2() + b.later() + b.plain() + b.out() + B<char>::Nested().get() +
         B<char>::Inner().get();
}

// The explicit instantiation definition emits the unmarked members
// weak_odr and leaves the marked ones out.
template struct B<int>;
