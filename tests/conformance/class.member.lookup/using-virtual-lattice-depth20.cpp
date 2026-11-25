// [class.member.lookup]/5, [class.mi]/4, [namespace.udecl]/1: a virtual base
// is one subobject however many paths reach it, so its lookup set is
// computed once. A lattice 20 virtual bases deep has 2^20 paths from L20a
// to L0a but 41 classes: the using-declaration's lookup, the member
// accesses and the overload sets below visit each class once (WP-3404;
// kiln searched every path, 26 s at depth 10). Reduced from real code.
//@ expect: compile-ok
namespace n {
struct L0a { void f(); void g(int); };
struct L0b : virtual L0a {};
struct L1a : virtual L0a, virtual L0b {};
struct L1b : virtual L0a, virtual L0b {};
struct L2a : virtual L1a, virtual L1b {};
struct L2b : virtual L1a, virtual L1b {};
struct L3a : virtual L2a, virtual L2b {};
struct L3b : virtual L2a, virtual L2b {};
struct L4a : virtual L3a, virtual L3b {};
struct L4b : virtual L3a, virtual L3b {};
struct L5a : virtual L4a, virtual L4b {};
struct L5b : virtual L4a, virtual L4b {};
struct L6a : virtual L5a, virtual L5b {};
struct L6b : virtual L5a, virtual L5b {};
struct L7a : virtual L6a, virtual L6b {};
struct L7b : virtual L6a, virtual L6b {};
struct L8a : virtual L7a, virtual L7b {};
struct L8b : virtual L7a, virtual L7b {};
struct L9a : virtual L8a, virtual L8b {};
struct L9b : virtual L8a, virtual L8b {};
struct L10a : virtual L9a, virtual L9b {};
struct L10b : virtual L9a, virtual L9b {};
struct L11a : virtual L10a, virtual L10b {};
struct L11b : virtual L10a, virtual L10b {};
struct L12a : virtual L11a, virtual L11b {};
struct L12b : virtual L11a, virtual L11b {};
struct L13a : virtual L12a, virtual L12b {};
struct L13b : virtual L12a, virtual L12b {};
struct L14a : virtual L13a, virtual L13b {};
struct L14b : virtual L13a, virtual L13b {};
struct L15a : virtual L14a, virtual L14b {};
struct L15b : virtual L14a, virtual L14b {};
struct L16a : virtual L15a, virtual L15b {};
struct L16b : virtual L15a, virtual L15b {};
struct L17a : virtual L16a, virtual L16b {};
struct L17b : virtual L16a, virtual L16b {};
struct L18a : virtual L17a, virtual L17b {};
struct L18b : virtual L17a, virtual L17b {};
struct L19a : virtual L18a, virtual L18b {};
struct L19b : virtual L18a, virtual L18b {};
struct L20a : virtual L19a, virtual L19b {};
struct L20b : virtual L19a, virtual L19b {};
struct D : L20a {
  void f(int);
  using L20a::f;
  using L20a::g;
};
struct E : L20a, L20b {};
} // namespace n
void h(n::D &d, n::E &e) {
  d.f(1);
  d.f();
  d.g(2);
  e.f();
  e.n::L0a::f();
  e.L10b::g(3);
}
