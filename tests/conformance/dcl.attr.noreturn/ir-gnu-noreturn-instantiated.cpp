// GNU `noreturn` on a member of a class template survives instantiation
// (clang's `TransformFunctionProtoType` keeps the `ExtInfo`): a call to
// the instantiated member, directly or through a pointer to it, is a
// `noreturn` call followed by `unreachable`, and a function ending in it
// needs no `return`. kiln dropped the bit from the instantiated member's
// type, so the call returned and fell off the end.
//@ expect: ir
//@ min-oracle: 22
//@ strict-diags
template <bool B> struct S {
  static void e() noexcept __attribute__((noreturn));
  void g() __attribute__((noreturn));
  [[gnu::noreturn]] static void h() noexcept(B);
};
int f1() { S<true>::e(); }
int f2(S<false> &s) { s.g(); }
int f3() {
  auto p = &S<true>::h;
  p();
}
int f4(bool b) {
  if (b)
    return 1;
  S<false>::h();
}
