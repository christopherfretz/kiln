// [dcl.attr.nouniqueaddr]/2, [basic.types.general]/2: a union with a
// non-POD member has a data size short of its size; a potentially-
// overlapping member of that union type is lowered as the union's `.base`
// type (clang 19 lowerUnion(isNonVirtualBaseType)) so that a following
// member can live in its tail padding.
struct B {
  constexpr B() : b(false) {}
  constexpr B(const B &) : B() {}
  alignas(2) bool b;
};
union U {
  constexpr U() : d() {}
  [[no_unique_address]] B d;
};
struct R {
  constexpr R() : u(), f(true) {}
  [[no_unique_address]] U u;
  [[no_unique_address]] bool f;
};
static_assert(sizeof(R) == 2);
int use(R &r) { return r.f + r.u.d.b; }
int main() {
  R r;
  return use(r) == 1 ? 0 : 1;
}
