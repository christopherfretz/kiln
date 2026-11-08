// [temp.dep.type]/6: in a base-specifier, `node::scoped_type` with `node` a member class of the
// current instantiation whose base is dependent names a member of an unknown specialization; it
// is looked up at instantiation (oneTBB's `const_accessor : private node::scoped_type`).
//@ expect: run
//@ stdout: 8
#include <cstdio>
struct M { struct scoped_lock { int held = 7; bool try_acquire() { return true; } }; };
template <class Mx> struct node_base { using scoped_type = typename Mx::scoped_lock; };
template <class A, class Mx> struct base { using nb = node_base<Mx>; };
template <class A, class Mx = M> struct map : base<A, Mx> {
  using base_type = base<A, Mx>;
  using node_base = typename base_type::nb;
  class node : public node_base { public: int v = 3; };
  class const_accessor : private node::scoped_type {
    friend struct map;
  public:
    int get() { return this->held + (node::scoped_type::try_acquire() ? 1 : 0); }
  };
};
int main() { map<int>::const_accessor a; std::printf("%d\n", a.get()); }
