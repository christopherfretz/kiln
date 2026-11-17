// [expr.ref]/4: the class of a member access must be complete "unless the
// class member access appears in the definition of that class". A member
// function's trailing return type, instantiated with its class, calls a
// member template through `declval<C const&>()`: deducing it substitutes
// the template's own trailing `decltype(a_ + b_)` (an implicit `this->a_`
// written in the class's definition), which is valid although the class is
// still being instantiated. Reduced from real code (range-v3's
// `iter_zip_with_view::cursor::move()` calling `move_()`).
//@ args: -std=c++20
template <class T> T&& declval();
template <int... I> struct seq {};

template <class X>
struct View {
  template <bool Const>
  struct cursor {
    X a_, b_;
    template <int... Is>
    auto move_(seq<Is...>) const -> decltype(a_ + b_) { return a_ + b_; }
    auto move() const -> decltype(declval<cursor const&>().move_(seq<0, 1>{})) {
      return move_(seq<0, 1>{});
    }
  };
};
View<int>::cursor<false> c{1, 2};
int r = c.move();

template <class T>
struct Plain {
  T a_;
  template <class U>
  auto get(U) const -> decltype(a_) { return a_; }
  auto use() const noexcept(noexcept(declval<Plain const&>().get(0)))
      -> decltype(declval<Plain const&>().get(0)) {
    return get(0);
  }
};
Plain<long> p{3};
long q = p.use();
