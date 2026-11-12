namespace pa {
template <class T>
constexpr T &&fwd([[clang::lifetimebound]] T &t) noexcept {
  return static_cast<T &&>(t);
}
inline const int &pick(const int &a [[clang::lifetimebound]],
                       [[maybe_unused]] int unused) {
  return a;
}
struct S {
  int v;
  const int &get(const int &d [[clang::lifetimebound]]) const {
    return v ? v : d;
  }
};
}
