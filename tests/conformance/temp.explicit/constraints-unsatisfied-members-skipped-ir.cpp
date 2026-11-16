// [temp.explicit]/10: the explicit instantiation definitions of
// `File<A>` and `File<B>` define exactly the members whose constraints
// are satisfied: every member of `File<A>`, only the unconstrained ones of
// `File<B>` (the same `weak_odr` definitions as clang). Found building an
// open-source linker.
// The IR is release-specific (the datalayout line and frame-pointer attributes
// differ between clang 19 and 22): pinned to clang 22 here and to clang 19 in its twin
// constraints-unsatisfied-members-skipped-ir-19.cpp (a clang compiler under test is never
// compared with the other release's oracle).
//@ std: c++20
//@ expect: ir
//@ requires: clang>=22
template <typename E> concept has_abi = requires { E::abi; };
struct A { static constexpr int abi = 1; };
struct B {};
template <typename E> struct File {
  int f() requires has_abi<E>;
  int g() { return 0; }
  File() requires has_abi<E> {}
  File(int) {}
  operator int() requires has_abi<E> { return E::abi; }
  static int sv;
  struct N {
    int h() requires has_abi<E> { return E::abi; }
    int k() { return 1; }
  };
};
template <typename E> int File<E>::f() requires has_abi<E> { return E::abi; }
template <typename E> int File<E>::sv = 3;
template struct File<A>;
template struct File<B>;
