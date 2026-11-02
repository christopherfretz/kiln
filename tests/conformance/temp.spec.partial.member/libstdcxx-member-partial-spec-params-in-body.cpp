// [temp.spec.partial.member], [temp.local]/8, [class.mem.general]/7: the
// template parameters of a member class template partial specialization
// are in scope in the bodies of its member functions, which are parsed
// after the enclosing class is complete (libstdc++ bits/stl_list.h:189
// `_Ptr_cmp<_Iter, void>`, chrono:2449 `__subseconds<duration<_Rep,
// _Period>>`; docs/status/libstdcxx.md cluster N, WP-332).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3 7
extern "C" int printf(const char *, ...);
struct L {
  template <class I, class P> struct cmp {};
  template <class I> struct cmp<I, void> { int f() { return I(1); } };
};
template <class V> struct M {
  template <class I, class P> struct cmp {};
  template <class I> struct cmp<I, void> {
    int f() { return I(2) + g(); }
    static constexpr int g() { return sizeof(V) - sizeof(V); }
  };
  template <class R, class P> struct dur {};
  template <class R> struct sub;
  template <class R, class P> requires (sizeof(R) > 1)
  struct sub<dur<R, P>> {
    unsigned char r{3};
    explicit operator dur<R, P>() const noexcept { return dur<R, P>(); }
    R get() const { return R(r); }
  };
};
template <class T> struct N {
  template <int K> struct v { static int get() { return K; } };
  template <int K> requires (K > 5) struct v<K> { static int get() { return K + T(0); } };
};
int main() {
  M<int>::sub<M<int>::dur<int, char>> s;
  (void)static_cast<M<int>::dur<int, char>>(s);
  printf("%d %d %d %d\n", L::cmp<int, void>().f(), M<long>::cmp<int, void>().f(), s.get(),
         N<int>::v<7>::get());
}
