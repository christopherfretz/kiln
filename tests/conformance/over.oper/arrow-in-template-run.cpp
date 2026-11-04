// [temp.res.general]/6, [over.ref]/1, [unique.ptr.single.observers],
// [util.smartptr.shared.obs]: `->` through `std::unique_ptr`,
// `std::shared_ptr`, a map iterator and a user-defined chain, resolved in
// a function template's definition and rebuilt at instantiation, run.
//@ args: -stdlib=libc++
//@ expect: run
//@ stdout: unique 1 shared 2 iter 3 chain 4
//@ stdout: mp 5 comma 67
#include <cstdio>
#include <map>
#include <memory>
#include <vector>

struct Opt {
  int id;
  int m() const { return id; }
  std::unique_ptr<Opt> clone() const { return std::make_unique<Opt>(id + 1); }
};
struct Box {
  Opt *o;
  Opt *operator->() const { return o; }
};
struct Wrap {
  Opt *o;
  Box operator->() const { return Box{o}; }
};
struct MP {
  Opt *o;
  int operator->*(int Opt::*m) const { return o->*m; }
};
struct Num {
  int v;
};
Num operator,(Num l, Num r) { return Num{l.v * 10 + r.v}; }

template <typename T> void f(T *app) {
  const std::unique_ptr<Opt> &u = app->v[0];
  std::printf("unique %d ", u->m());
  std::shared_ptr<Opt> s = app->m.begin()->second;
  std::printf("shared %d ", s->m());
  auto it = app->m.begin();
  std::printf("iter %d ", it->second->clone()->m());
  Opt o{4};
  Wrap w{&o};
  std::printf("chain %d\n", w->m());
  Opt five{5};
  MP mp{&five};
  std::printf("mp %d ", mp->*&Opt::id);
  Num n = (Num{6}, Num{7});
  std::printf("comma %d\n", n.v);
}

struct App {
  std::vector<std::unique_ptr<Opt>> v;
  std::map<int, std::shared_ptr<Opt>> m;
};

int main() {
  App a;
  a.v.push_back(std::make_unique<Opt>(1));
  a.m[0] = std::make_shared<Opt>(2);
  f(&a);
}
