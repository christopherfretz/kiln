// [class.qual]/2.2: in a class's using-declarator whose nested-name-specifier
// nominates a class C, a name that is C's injected-class-name names C's
// constructors however the qualifier is spelled: `using BC::B;` with
// `typedef B<char> BC` inherits `B<char>`'s constructors, as rocksdb's
// `using std::regex::basic_regex;` (spec/landed/sema/WP-724.md). WP-724.
//@ expect: run
//@ strict-diags
//@ stdout: ok 7
extern "C" int printf(const char *, ...);

template <class T> struct B {
  int v;
  B(int x) : v(x) {}
  B(const char *, int y) : v(y + 1) {}
};
typedef B<char> BC;
struct D : BC {
  using BC::B;
};
using BL = B<long>;
struct E : BL {
  using BL::B;
};
struct F : B<int> {
  using B<int>::B;
};

int main() {
  D d(3);
  D e("x", 5);
  E f(1);
  F g("y", 0);
  printf("ok %d\n", d.v + e.v + f.v + g.v - 4);
  return 0;
}
