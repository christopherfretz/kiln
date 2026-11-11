// [temp.inst]/3 with [class.mem.general]/8: a member class template's
// specialization implicitly instantiated by a member declaration of the
// enclosing class precedes the parsing of the template's default member
// initializers (at the end of the outermost class); a constructor or
// aggregate initialization using one later still instantiates it. Reduced
// from the reference workload: a `Field<bool>` whose user-provided
// constructor left the `bool isPresent_{false}` member uninitialized.
//@ expect: run
//@ stdout: 0 0 7 5
#include <cstdio>
#include <cstring>
#include <new>

struct Outer {
  template <class T> struct Field {
    Field() { std::memset(&value_, 0, sizeof(value_)); }
    bool isPresent_{false};
    T value_;
  };
  template <class T> struct Agg {
    T a = 7;
    T b;
  };
  Field<bool> f_;
  Agg<int> g_;
  Outer() : g_{.b = 5} {}
};

__attribute__((noinline)) void fill(void *p, unsigned n) { std::memset(p, 0xff, n); }

int main() {
  alignas(Outer) unsigned char buf[sizeof(Outer)];
  fill(buf, sizeof buf);
  Outer *o = new (buf) Outer;
  Outer::Field<bool> local;
  std::printf("%d %d %d %d\n", o->f_.isPresent_, local.isPresent_, o->g_.a, o->g_.b);
}
