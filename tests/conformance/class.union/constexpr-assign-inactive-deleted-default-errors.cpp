// [class.union.general]/6: a union member whose class's trivial default
// constructor is deleted (explicitly, or a defaulted one defined as
// deleted) is not in S(E1), so assigning through it while inactive does
// not begin its lifetime and is not a constant expression. clang 22
// accepts both (it tests only for a trivial default constructor); GCC 16.1
// rejects them, and the text decides.
//@ skip(clang): clang 22 does not check that the trivial default constructor is not deleted
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:3 warning "explicitly defaulted default constructor is implicitly deleted [-Wdefaulted-function-deleted]"
//@ diag: 26:7 note "default constructor of 'DefaultedDel' is implicitly deleted because field 'd' has a deleted default constructor"
//@ diag: 23:3 note "'Del' has been explicitly marked deleted here"
//@ diag: 27:20 note "replace 'default' with 'delete'"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 37:11 note "assignment to member 'del' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 40:15 note "in call to 'deleted()'"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:12 note "assignment to member 'dd' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 46:15 note "in call to 'defaulted_deleted()'"
struct Del {
  int x;
  Del() = delete;
};
struct DefaultedDel {
  Del d;
  DefaultedDel() = default;
};
union U {
  int i;
  Del del;
  DefaultedDel dd;
  constexpr U() : i(0) {}
};
constexpr int deleted() {
  U u;
  u.del.x = 1;
  return u.del.x;
}
static_assert(deleted() == 1);
constexpr int defaulted_deleted() {
  U u;
  u.dd.d.x = 1;
  return 1;
}
static_assert(defaulted_deleted() == 1);
