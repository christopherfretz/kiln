// [dcl.spec.auto.general]/5: a placeholder cannot be the type of a
// non-static data member (a static member may use one).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:3 error "'auto' not allowed in non-static struct member"
//@ diag: 11:3 error "'decltype(auto)' not allowed in non-static struct member"
//@ diag: 14:11 error "'auto' not allowed in non-static class member"
//@ diag: 15:11 error "'auto' not allowed in non-static union member"
struct S {
  auto a = 1;
  decltype(auto) b = 2;
  static const auto c = 3;
};
class C { auto *p = nullptr; };
union U { auto u = 0; };
static_assert(__is_same(decltype(S::c), const int));
