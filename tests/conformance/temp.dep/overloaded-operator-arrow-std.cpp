// [temp.res.general]/6, [over.ref]/1, [unique.ptr.single.observers],
// [util.smartptr.shared.obs]: inside a function template, `x->m` with a
// non-dependent `std::unique_ptr` / `std::shared_ptr` / map iterator
// operand is resolved through `operator->` at the definition; the
// instantiation forms the chain again from the class object (the `const`
// member function adds a qualification conversion of the returned pointer
// between the call and the access). A dependent operand finds `operator->`
// at instantiation.
//@ args: -stdlib=libc++
//@ expect: compile-ok
//@ strict-diags
#include <map>
#include <memory>
#include <vector>

struct Opt {
  int m() const { return 1; }
  int n() { return 2; }
  int v = 3;
};
using Opt_p = std::unique_ptr<Opt>;
struct App {
  std::vector<Opt_p> v;
  std::map<int, std::shared_ptr<Opt>> m;
};

template <typename T> int f(T *p) {
  const Opt_p &opt = p->v[0];
  int r = opt->m() + opt->n() + opt->v;
  Opt_p local = std::make_unique<Opt>();
  r += local->m() + local->v;
  auto it = p->m.begin();
  r += it->second->m() + it->second->v;
  std::map<int, std::shared_ptr<Opt>>::const_iterator ci = p->m.cbegin();
  r += ci->second->m() + ci->first;
  return r;
}

// A dependent smart-pointer operand: `operator->` is found at instantiation.
template <typename P> int dep(const P &p) { return p->m() + p->v; }

int g() {
  App a;
  a.v.push_back(std::make_unique<Opt>());
  a.m[4] = std::make_shared<Opt>();
  return f(&a) + dep(a.v[0]) + dep(a.m[4]);
}
