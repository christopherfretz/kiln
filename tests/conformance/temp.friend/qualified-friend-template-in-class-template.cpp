// [temp.friend]/1, [dcl.meaning.general]/2.3, [temp.over.link]/6: a qualified friend function
// template declared in a class template names the namespace-scope template it redeclares once its
// parameters are renamed by position (`U` is one level deeper in the friend than in the namespace
// template); the match is made when the class is instantiated, so the friend grants access to
// that specialization. The parameter type may name the enclosing class template through its
// injected-class-name (`pc<U>`), and the namespace may be reached through an inline namespace.
// libc++ 19 experimental/propagate_const:196 (get_underlying), WP-273. Verified against clang 19.
//@ expect: compile-ok
namespace ns { inline namespace v2 {
template <class T> class pc;
template <class U> constexpr const U& gu(const pc<U>& p) noexcept;
template <class U> constexpr U& gu(pc<U>& p) noexcept;
template <class T> class pc {
  T t_;
public:
  template <class U> friend constexpr const U& ns::v2::gu(const pc<U>& p) noexcept;
  template <class U> friend constexpr U& v2::gu(pc<U>& p) noexcept;
  constexpr pc(T t) : t_(t) {}
};
template <class U> constexpr const U& gu(const pc<U>& p) noexcept { return p.t_; }
template <class U> constexpr U& gu(pc<U>& p) noexcept { return p.t_; }

// A friend naming the enclosing class's parameter alongside its own: it redeclares
// `mix` in the specialization `m<int>` only.
template <class B> int mix(int, B*);
template <class T> class m {
  int v = 7;
  template <class B> friend int ns::mix(T, B*);
public:
  m() = default;
};
template <class B> int mix(int, B*) { return m<int>().v; }

// A qualified non-template friend whose type depends on the class's parameter.
int dep(long);
template <class T> class d {
  static constexpr int v = 5;
  friend int ns::dep(T);
};
int dep(long) { return d<long>::v; }
}}
constexpr int f() {
  ns::pc<int> p(3);
  const ns::pc<int>& c = p;
  return ns::gu(p) + ns::gu(c);
}
static_assert(f() == 6);
int g() { int* q = nullptr; return ns::mix(1, q) + ns::dep(2); }

// From a class template in another namespace: the instantiated friend redeclares `N::h` and
// `N::k`, not a function of the class's own namespace.
namespace N { template <class T> int h(T); int k(long); }
template <class X> class R {
  static const int v = 1;
  template <class T> friend int N::h(T);
  friend int N::k(long);
};
template <class T> int N::h(T) { return R<int>::v; }
int N::k(long) { return R<long>::v; }
int r() { return N::h(1) + N::k(1); }
