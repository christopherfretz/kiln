// [over.over]/1, [dcl.init.ref]/5.4.1, [expr.static.cast]/4: a class type
// is not an [over.over] target. An overload set initializing a class
// object, or a reference to one through a temporary, is the argument of
// the class's converting constructor taking a function pointer (folly's
// `const dynamic& v = dynamic::object` default arguments, with
// `dynamic(ObjectMaker (*)())`).
//@ expect: run
//@ stdout: 1 1 2 1 1
#include <cstdio>

struct OM {
  int x;
};
OM object() { return {1}; }
OM object(int, int) { return {2}; }

struct dyn {
  int v;
  dyn(OM (*f)()) : v(f().x) {}
  dyn(OM const&) = delete;
  dyn(OM&& o) : v(o.x) {}
  static int get(const dyn& d = object) { return d.v; }
};

int take(const dyn& d) { return d.v; }

int main() {
  const dyn& r = object;
  dyn m = static_cast<dyn>(object);
  std::printf("%d %d %d %d %d\n", r.v, take(object), take(object(0, 0)), m.v, dyn::get());
}
