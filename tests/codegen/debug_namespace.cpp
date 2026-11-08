// Debug info (WP-814): namespaces as scopes of variables and types.
//@ args: -g
namespace ns {
int g = 7;
namespace inner {
struct T {
  int v;
};
} // namespace inner
} // namespace ns
int f(ns::inner::T t) { return t.v + ns::g; }
