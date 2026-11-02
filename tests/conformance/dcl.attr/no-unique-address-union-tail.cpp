// [dcl.attr.nouniqueaddr]/2, [intro.object]/9: a member placed in the tail
// padding of a preceding [[no_unique_address]] union member is a distinct
// subobject; constant evaluation of its initialization must address it,
// not the union's active member at an overlapping offset. Verified against
// clang 19.
//@ expect: run
//@ strict-diags
struct B {
  constexpr B() : b(false) {}
  constexpr B(const B &) : B() {}
  alignas(2) bool b;
};
union U {
  constexpr U() : d() {}
  constexpr ~U() {}
  [[no_unique_address]] B d;
};
struct R {
  constexpr R() : u(), f(true) {}
  [[no_unique_address]] U u;
  [[no_unique_address]] bool f;
};
static_assert(sizeof(R) == 2);
constexpr bool test() {
  R r;
  return r.f && !r.u.d.b;
}
static_assert(test());
int main() {
  R r;
  return test() && r.f && !r.u.d.b ? 0 : 1;
}
