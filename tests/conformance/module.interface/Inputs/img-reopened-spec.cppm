export module imgrs;
export {
namespace rs {
template <class C> struct traits;
template <> struct traits<char>;
}
namespace rs {
template <> struct traits<char> {
  using type = char;
  static constexpr int size() { return 1; }
};
}
}
