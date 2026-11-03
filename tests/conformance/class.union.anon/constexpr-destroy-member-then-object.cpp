// [class.union.general]/2, [basic.life]/1, [expr.const]/5.18: ending the
// lifetime of the active member of an anonymous union (a pseudo-destructor
// call through std::destroy_at) leaves the union with no active member; the
// enclosing object stays within its lifetime, so destroying it afterwards
// is constant. libc++ 22's forward_list/list nodes do exactly this in
// __delete_node (WP-446 cluster "list node destruction"). Destroying the
// object twice is still diagnosed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:58 note "destruction of object outside its lifetime is not allowed in a constant expression"
//@ diag: 58:3 note "in call to 'destroy_at<Node<int>>(&{*new Node<int>[1]#0}[0])'"
//@ diag: 62:15 note "in call to 'destroy_twice()'"
using size_t = decltype(sizeof(0));
void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T> struct allocator {
  constexpr T *allocate(size_t n) { return static_cast<T *>(operator new(n * sizeof(T))); }
  constexpr void deallocate(T *p, size_t) { operator delete(p); }
};
template <class T, class... A> constexpr T *construct_at(T *p, A... a) {
  return ::new ((void *)p) T(a...);
}
template <class T> constexpr void destroy_at(T *p) { p->~T(); }
} // namespace std

struct Link {
  Link *next;
  constexpr Link(Link *n) : next(n) {}
};

template <class T> struct Node : Link {
  union {
    T value;
  };
  constexpr Node(Link *n) : Link(n) {}
  constexpr T &get() { return value; }
};

constexpr int member_then_node() {
  std::allocator<Node<int>> a;
  Node<int> *p = a.allocate(1);
  std::construct_at(p, nullptr);
  std::construct_at(&p->get(), 5);
  int v = p->get();
  std::destroy_at(&p->get());
  std::destroy_at(p);
  a.deallocate(p, 1);
  return v;
}
static_assert(member_then_node() == 5);

constexpr bool destroy_twice() {
  std::allocator<Node<int>> a;
  Node<int> *p = a.allocate(1);
  std::construct_at(p, nullptr);
  std::destroy_at(p);
  std::destroy_at(p);
  a.deallocate(p, 1);
  return true;
}
static_assert(destroy_twice());

int main() {}
