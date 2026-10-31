// [temp.spec.partial.match]: X<R(Args...)> matches a function type.
template <class T> struct sig;
template <class R, class... A> struct sig<R(A...)> {
  static constexpr int n = sizeof...(A);
  using ret = R;
};
static_assert(sig<int(char, long)>::n == 2);
static_assert(sig<int()>::n == 0);
static_assert(__is_same(sig<void(int)>::ret, void));
