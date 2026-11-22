// Per-specialization entries (spec/ast.md §18.9.1, §18.9.5, WP-2224): a
// specialization prints with its canonical template arguments, whatever
// spelling instantiated it first. This file and inst-spelling-a.cpp are
// the same program but for the order of the statements in `run`, which
// name each specialization through the class's own name first here and
// through an alias first there ([temp.type]/1: the same specialization);
// their goldens differ only in the lines of `run`.
//@ written-refs: inst-spelling-b.refs
namespace lib {
struct Item {
  int size() const { return 1; }
  using kind = int;
};
} // namespace lib
using Alias = lib::Item;

template <class T> int measure(const T &t) {
  typename T::kind k = 0;
  return t.size() + static_cast<int>(k);
}
template <class T> struct Holder {
  T t;
  int get() const { return t.size(); }
};

int run() {
  int c = measure<lib::Item>(lib::Item{});
  int d = Holder<lib::Item>{}.get();
  int a = measure<Alias>(Alias{});
  int b = Holder<Alias>{}.get();
  return a + b + c + d;
}
