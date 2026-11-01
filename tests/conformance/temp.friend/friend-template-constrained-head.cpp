// [temp.friend]/1, [temp.over.link]/6: a friend class template declared in a class template has a
// head one level deeper than the namespace-scope template's, which may name the enclosing
// template's parameters; it is compared when the class template is instantiated, not when it is
// defined (`ev` and `jv` are not instantiated). Shapes from libc++ 19: a constrained class template befriending itself
// (`common_iterator`, `counted_iterator`, the test suite's `cpp20_random_access_iterator`), a
// non-type parameter whose type is a parameter (`__deque_iterator`), dependent friends of one name
// with different arity (`split_view` and `elements_view`'s `__iterator`), a constraint naming an
// enclosing parameter, and a namespace-scope redeclaration after a dependent friend.
//@ expect: run
//@ strict-diags
template <class T> concept Ptr = __is_pointer(T);
template <class T, class U> concept Same = __is_same(T, U);
template <class T, class U> concept Not = !__is_same(T, U);

template <Ptr I, Not<I> S>
  requires(sizeof(I) == sizeof(void*))
class ci {
  I i_;
  template <Ptr I2, Not<I2> S2>
    requires(sizeof(I2) == sizeof(void*))
  friend class ci;
public:
  constexpr explicit ci(I i) : i_(i) {}
  template <class I2, class S2>
  constexpr ci(const ci<I2, S2>& o) : i_(o.i_) {}
  constexpr I get() const { return i_; }
};

template <Ptr I>
class cnt {
  I i_ = nullptr;
  template <Ptr>
  friend class cnt;
public:
  cnt() = default;
  template <class J>
  cnt(const cnt<J>& o) : i_(o.i_) {}
};

template <class V, class D, D B>
struct dit {
  template <class V2, class D2, D2 B2>
  friend struct dit;
  template <class V2, class D2, D2 B2>
  int peek(const dit<V2, D2, B2>& o) const { return o.secret; }
private:
  int secret = static_cast<int>(B);
};

namespace rng {
template <class T> struct ev { template <bool> friend struct it; };
template <class T> struct sv { template <class, class> friend struct it; };
template <class T> struct jv { template <bool> friend struct it2; };
template <long N> struct it2;
}

template <class T, class U> concept Conv = __is_convertible(T, U);
template <Conv<long> T> struct q { static constexpr int v = 7; };
template <class U> struct y {
  template <Conv<U> T> friend struct q;
};

template <Ptr T> struct s3 { static constexpr int v = 3; };
template <class X> struct a3 { template <Ptr T> friend struct s3; };
template <Ptr T> struct s3;

int main() {
  int x = 5;
  ci<int*, long> a(&x);
  ci<const int*, long> b(a);
  cnt<int*> c;
  cnt<const int*> d(c);
  (void)d;
  dit<int, int, 4> e;
  dit<char, long, 9> f;
  rng::sv<int> h;
  (void)h;
  y<long> k;
  (void)k;
  a3<int> m;
  (void)m;
  return (*b.get() == 5 && e.peek(f) == 9 && q<int>::v + s3<int*>::v == 10) ? 0 : 1;
}
