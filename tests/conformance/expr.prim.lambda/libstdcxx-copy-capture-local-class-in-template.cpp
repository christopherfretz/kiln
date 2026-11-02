// [expr.prim.lambda.capture]/10, /15, [temp.dep.type]/7: in a template
// definition, a by-copy capture (simple, implicit or init-capture) or a
// structured binding of an object whose type is a local class of the
// template, or a member class of the current instantiation, is
// initialized when the template is instantiated: the class has no
// implicit copy constructor before then (clang
// `CanDeclareSpecialMemberFunction`), so nothing is checked in the
// definition (libstdc++ pstl/algorithm_impl.h:1978 `_PartitionRange`
// captured by `[__init, ...]` in `__parallel_partition_copy`; WP-332).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 7 7 7 7 9 5
extern "C" int printf(const char *, ...);

template <class It> int simple(It b) {
  struct PR { It a; };
  PR v1{b};
  auto g = [v1]() { return *v1.a; };
  return g();
}

template <class It> int implicit(It b) {
  struct PR { It a; };
  PR v1{b};
  auto g = [=]() { return *v1.a; };
  return g();
}

template <class It> int init_capture(It b) {
  struct PR { It a; };
  PR v1{b};
  auto g = [w = v1]() { return *w.a; };
  return g();
}

template <class It> int binding(It b) {
  struct PR { It a; };
  PR arr[1]{{b}};
  auto [e] = arr;
  return *e.a;
}

template <class T> struct Outer {
  struct In { T v; };
  int get(T t) {
    In i{t};
    auto g = [i] { return i.v + 2; };
    return g();
  }
};

template <class T> int nested(T t) {
  struct PR { T v; };
  PR p{t};
  auto g = [p] { return [p] { return p.v; }(); };
  return g();
}

int main() {
  int x = 7;
  printf("%d %d %d %d %d %d\n", simple(&x), implicit(&x), init_capture(&x),
         binding(&x), Outer<int>().get(7), nested(5));
  return 0;
}
