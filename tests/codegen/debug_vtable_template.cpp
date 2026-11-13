// Debug info (WP-1372): class template specializations with a virtual
// base. Node<int> and Leaf<short> are homed by their own vtables; Node<short>
// has no vtable here, only the construction vtable of Leaf<short>'s VTT,
// which completes it as clang's GenerateConstructionVTable does.
//@ args: -g
namespace t {
struct Root {
  virtual int id() const { return 0; }
  int r = 1;
};
template <typename T> struct Node : virtual Root {
  T value{};
  virtual T get() const { return value; }
};
template <typename T> struct Leaf : Node<T> {
  T get() const override { return 2; }
};
} // namespace t
int run() {
  t::Node<int> a;
  t::Leaf<short> b;
  return a.get() + b.get() + b.id();
}
