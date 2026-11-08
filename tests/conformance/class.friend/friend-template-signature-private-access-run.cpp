// [class.friend]/2, /7, [temp.deduct.general]/8: the signature of a
// friend function template defined in (or first declared in) a class is
// substituted with that class's access, so a trailing return type or a
// noexcept-specifier naming a private member is not a substitution
// failure. This is how a sender/receiver library's `tag_invoke(connect,
// sender, receiver)` friend names the sender's private state (WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 1 2 3 4 5
extern "C" int printf(const char *, ...);

struct C {
private:
  int priv_ = 1;
  template <class S>
  friend auto get_a(S &&s) -> decltype((static_cast<S &&>(s).priv_)) {
    return static_cast<S &&>(s).priv_;
  }
  template <class S> friend auto get_b(S &&s) -> decltype(s.priv_);
};
template <class S> auto get_b(S &&s) -> decltype(s.priv_) { return s.priv_ + 1; }

template <class P> struct outer { class type; };
template <class P> class outer<P>::type {
  P pred_ = 3;

public:
  template <class S>
  friend auto get_c(S &&s) -> decltype((static_cast<S &&>(s).pred_)) {
    return static_cast<S &&>(s).pred_;
  }
};

template <class T> class D {
  T x_ = 4;
  template <class S>
  friend auto get_d(S &s) noexcept(noexcept(s.x_)) -> decltype(s.x_ + 1) { return s.x_ + 1; }

public:
  template <class S> friend auto get_e(S &s) -> decltype(s.x_) { return s.x_; }
};

template <class T> int probe(...) { return 0; }
template <class T> int probe(T &&t, decltype(get_e(t)) * = nullptr) { return 1; }

int main() {
  C c;
  outer<int>::type t;
  D<int> d;
  printf("%d %d %d %d %d\n", get_a(c), get_b(c), get_c(t), get_e(d), get_d(d) * probe<D<int> &>(d));
}
