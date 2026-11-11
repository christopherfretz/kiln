#ifndef DEFAULTED_IN_TEMPLATE_H
#define DEFAULTED_IN_TEMPLATE_H
struct tag {
  explicit tag() = default;
};
template <class E> int make() { return E{tag{}}.v; }
struct pol {
  int v = 3;
  constexpr explicit pol(tag) {}
};
#endif
