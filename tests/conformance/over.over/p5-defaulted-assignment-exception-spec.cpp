// [over.over]/5 with [except.spec]/8: the implicit exception
// specification of a defaulted assignment operator, computed on demand,
// is part of the type an overload set's target is matched against; the
// function pointer conversion drops a `noexcept`, never adds one.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 42:32 error "address of overloaded function 'operator=' does not match required type 'H &(H &&) noexcept'"
//@ diag: 25:6 note "candidate function has type mismatch at 1st parameter (expected 'H &&' but has 'const H &')"
//@ diag: 26:6 note "candidate function has different exception specification"

template <class T> struct Foo {
  Foo &operator=(Foo &&) = default;
  T data;
};
struct G {
  G &operator=(G &&) = default;
  int data;
};
struct Throws {
  Throws &operator=(Throws &&);
};
struct H {
  H &operator=(const H &) = delete;
  H &operator=(H &&) = default;
  Throws t;
};

template <class T> int consume(Foo<T> &(Foo<T>::*)(Foo<T> &&)) { return 1; }
template <class T> int consume(Foo<T> &(Foo<T>::*)(Foo<T> &&) noexcept) { return 2; }

void plain(G &(G::*)(G &&)) {}
void strict(G &(G::*)(G &&) noexcept) {}

int main() {
  consume(&Foo<int>::operator=);
  plain(&G::operator=);
  strict(&G::operator=);
  G &(G::*p)(G &&) noexcept = &G::operator=;
  H &(H::*q)(H &&) = &H::operator=;
  H &(H::*r)(H &&) noexcept = &H::operator=;
  (void)p, (void)q, (void)r;
}
