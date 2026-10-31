// [stmt.ranged]/1.3: a class range needs both member `begin` and `end`,
// else both are looked up by argument-dependent lookup; an incomplete
// class, a scalar or a pointer is not a range. A pointer to a range gets
// clang's dereference suggestion; a lone member `begin` is ignored (note).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:14 error "invalid range expression of type 'Empty'; no viable 'begin' function available"
//@ diag: 31:14 error "invalid range expression of type 'OnlyBegin'; no viable 'end' function available"
//@ diag: 27:6 note "candidate function not viable: no known conversion from 'OnlyBegin' to 'M &' for 1st argument"
//@ diag: 32:16 error "cannot use incomplete type 'Incomplete' as a range"
//@ diag: 22:8 note "forward declaration of 'Incomplete'"
//@ diag: 33:14 error "invalid range expression of type 'N::S'; no viable 'end' function available"
//@ diag: 34:14 error "invalid range expression of type 'Empty *'; no viable 'begin' function available"
//@ diag: 35:15 error "invalid range expression of type 'int'; no viable 'begin' function available"
//@ diag: 36:16 error "invalid range expression of type 'R *'; did you mean to dereference it with '*'?"
//@ diag: 37:15 error "invalid range expression of type 'std::nullptr_t'; no viable 'begin' function available"
//@ diag: 38:15 error "invalid range expression of type 'int (*)()'; no viable 'begin' function available"
//@ diag: 39:15 error "invalid range expression of type 'M'; no viable 'begin' function available"
//@ diag: 26:17 note "member is not a candidate because range type 'M' has no 'end' member"
struct Empty {};
struct OnlyBegin { int* begin(); };
struct Incomplete;
extern Incomplete inc;
namespace N { struct S {}; int* begin(S&); }
struct R { int* begin(); int* end(); };
struct M { int* begin(); };
int* end(M&);

void f(Empty e, OnlyBegin o, N::S s, Empty* p, R* rp, int (*fp)()) {
  for (int x : e) {}
  for (int x : o) {}
  for (int x : inc) {}
  for (int x : s) {}
  for (int x : p) {}
  for (auto x : 5) {}
  for (int x : rp) {}
  for (auto x : nullptr) {}
  for (auto x : fp) {}
  for (auto x : M{}) {}
}
