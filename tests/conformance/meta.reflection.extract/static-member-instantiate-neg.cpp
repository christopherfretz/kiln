// [temp.inst]/5 and /8, [meta.reflection.extract] (P2996R13;
// spec/reflection.md §3 "Reading a variable through a reflection", WP-638):
// `extract` of a static data member instantiates its initializer, so one
// ill-formed for it is diagnosed there (notes: the extract, then the calls).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 21:28 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 26:10 note "in instantiation of static data member 'bad<int>::x' requested here"
//@ diag: 29:15 note "in call to 'read()'"
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 26:10 note "initializer of 'x' is unknown"
//@ diag: 29:15 note "in call to 'read()'"
//@ diag: 21:24 note "declared here"
#include <meta>

template <class T>
struct bad {
  static constexpr int x = T::nope;
};

consteval int read() {
  auto s = std::meta::static_data_members_of(^^bad<int>, std::meta::access_context::unchecked());
  return std::meta::extract<int>(s[0]);
}

static_assert(read() == 0);
