// [meta.define.static] (P3491R3), spec/reflection-values.md §4: the
// ill-formed and throwing cases of reflect_constant_string,
// reflect_constant_array and the define_static_* functions, each
// diagnosed with a note.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-fail
//@ strict-diags
//@ diag: 57:21 error "call to consteval function 'std::meta::reflect_constant_string<const int (&)[2]>' is not a constant expression"
//@ diag: 57:21 note "in call to 'reflect_constant_string<const int (&)[2]>(ints)'"
//@ diag: 57:16 error "constexpr variable 'n1' must be initialized by a constant expression"
//@ diag: 57:21 note "in call to 'reflect_constant_string<const int (&)[2]>(ints)'"
//@ diag: 58:21 error "call to consteval function 'std::meta::reflect_constant_array<const NS (&)[1]>' is not a constant expression"
//@ diag: 58:21 note "in call to 'reflect_constant_array<const NS (&)[1]>(nss)'"
//@ diag: 58:16 error "constexpr variable 'n2' must be initialized by a constant expression"
//@ diag: 58:21 note "in call to 'reflect_constant_array<const NS (&)[1]>(nss)'"
//@ diag: 59:21 error "call to consteval function 'std::define_static_array<const K (&)[1]>' is not a constant expression"
//@ diag: 59:21 note "in call to 'define_static_array<const K (&)[1]>(ks)'"
//@ diag: 59:16 error "constexpr variable 'n3' must be initialized by a constant expression"
//@ diag: 59:21 note "in call to 'define_static_array<const K (&)[1]>(ks)'"
//@ diag: 60:21 error "call to consteval function 'locals' is not a constant expression"
//@ diag: 54:10 note "in call to 'define_static_array<std::span<const int *const>>(std::span<const int *const>(a, 1))'"
//@ diag: 60:21 note "in call to 'locals()'"
//@ diag: 52:7 note "declared here"
//@ diag: 60:16 error "constexpr variable 'n4' must be initialized by a constant expression"
//@ diag: 54:10 note "in call to 'define_static_array<std::span<const int *const>>(std::span<const int *const>(a, 1))'"
//@ diag: 60:21 note "in call to 'locals()'"
//@ diag: 52:7 note "declared here"
//@ diag: 61:21 error "call to consteval function 'std::define_static_object<K>' is not a constant expression"
//@ diag: 61:21 note "in call to 'define_static_object<K>(K{\"lit\"})'"
//@ diag: 61:16 error "constexpr variable 'n5' must be initialized by a constant expression"
//@ diag: 61:21 note "in call to 'define_static_object<K>(K{\"lit\"})'"
//@ diag: 62:21 error "call to consteval function 'std::define_static_object<NS>' is not a constant expression"
//@ diag: 62:21 note "in call to 'define_static_object<NS>(NS{})'"
//@ diag: 62:16 error "constexpr variable 'n6' must be initialized by a constant expression"
//@ diag: 62:21 note "in call to 'define_static_object<NS>(NS{})'"
#include "kiln_meta.h"
using namespace std::meta;

struct NS {
 private:
  int x = 0;

 public:
  constexpr NS() {}
};
struct K { const char* p; };
constexpr int ints[] = {1, 2};
constexpr NS nss[1] = {};
constexpr K ks[] = {{"lit"}};
consteval std::span<const int* const> locals() {
  int x = 1;
  const int* a[] = {&x};
  return std::define_static_array(std::span<const int* const>(a, 1));
}

constexpr info n1 = reflect_constant_string(ints);
constexpr info n2 = reflect_constant_array(nss);
constexpr auto n3 = std::define_static_array(ks);
constexpr auto n4 = locals();
constexpr auto n5 = std::define_static_object(K{"lit"});
constexpr auto n6 = std::define_static_object(NS{});
