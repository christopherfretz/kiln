// Per-specialization entries (spec/ast.md §18.9.2): in a member template
// of a class template ([temp.mem]/1) a dependent name in the body is
// resolved by the member template's specialization
// (`Box<Small>::convert<Wide>`), the innermost one, whether it depends on
// the inner parameter (`u.width()`) or only on the outer one
// (`value.bits()`); the outer class template specialization is the key's
// enclosing class (`Box<Small>`). A name in the member template's
// declaration (`typename T::tag`, the return type) is resolved twice:
// when the class is instantiated ([temp.inst]/3: the member template's
// declaration, keyed by `Box<Small>`) and when the member template's
// signature is substituted for its specialization ([temp.deduct]/5).
//@ written-refs: inst-nested.refs
struct Small {
  using tag = int;
  int bits() const { return 8; }
};
struct Wide {
  long width() const { return 64; }
};

template <class T> struct Box {
  T value;
  template <class U> typename T::tag convert(const U &u) const {
    return static_cast<typename T::tag>(value.bits() + u.width());
  }
};

int run() {
  Box<Small> b{};
  return b.convert(Wide{});
}
