// [class.this]/1 with the GNU `__restrict` method qualifier: `this` is
// `C * __restrict` in the body and in by-reference `[this]` captures, but a
// `[*this]` capture is a copy and is not restrict-qualified (WP-1091; clang
// SemaCXX/restrict-this.cpp).
//@ expect: compile-ok
struct C {
  int v;
  void f() __restrict {
    static_assert(__is_same(decltype(this), C *__restrict));
    (void)[this]() {
      static_assert(__is_same(decltype(this), C *__restrict));
      (void)[*this]() { static_assert(__is_same(decltype(this), const C *)); };
      (void)[*this]() mutable { static_assert(__is_same(decltype(this), C *)); };
    };
  }
  void g() const __restrict {
    static_assert(__is_same(decltype(this), const C *__restrict));
    static_assert(__is_same(decltype((v)), const int &));
    C *__restrict p = const_cast<C *>(this);
    (void)p;
  }
};
template <typename T> struct TC {
  void f() __restrict {
    static_assert(__is_same(decltype(this), TC<int> *__restrict));
  }
};
void use() { TC<int>{}.f(); }
