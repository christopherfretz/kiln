// [expr.prim.this]/3, [except.spec]/13: the noexcept-specifier of a
// constructor of a class template specialization, instantiated when
// needed, may name non-static members through the implicit `this`, as a
// member function's may (a sender library's operation state:
// `type(F2&& f, R2&& r) noexcept(noexcept(connect_inner_op(func_, ...)))`;
// WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 1 0 1 7 1
extern "C" int printf(const char *, ...);

template <class F> struct op {
  struct type {
    template <class R2> int conn(F &f, R2 &&r) noexcept(noexcept(f(r))) { return f(r); }
    F func_;
    int v;
    template <class F2, class R2>
    explicit type(F2 &&f, R2 &&r) noexcept(noexcept(conn(func_, static_cast<R2 &&>(r))))
        : func_(static_cast<F2 &&>(f)), v(conn(func_, r)) {}
    ~type() noexcept(noexcept(this->conn(func_, 0))) {}
  };
};

struct safe {
  int operator()(int x) noexcept { return x; }
};
struct risky {
  int operator()(int x) { return x; }
};

int main() {
  using S = op<safe>::type;
  using R = op<risky>::type;
  S s(safe{}, 7);
  printf("%d %d %d %d %d\n", noexcept(S(safe{}, 1)), noexcept(R(risky{}, 1)),
         noexcept(s.~S()), s.v, !noexcept(R(risky{}, 1).~R()));
}
