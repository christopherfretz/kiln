// [meta.reflection.result] (P2996R13), spec/reflection-values.md §3: every condition
// under which reflect_constant, reflect_object and reflect_function throw, each
// diagnosed with a note (the evaluation fails; std::meta::exception as a thrown
// object is WP-484's). r4 is the IS example's. reflect_function of a consteval
// function (n3) is valid: P4101R1 struck the rule that a constant expression may not
// refer to an immediate function (GCC 16.1 agrees; WP-1361).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-fail
//@ strict-diags
//@ diag: 77:16 error "constexpr variable 'r4' must be initialized by a constant expression"
//@ diag: 77:21 note "the value refers to a string literal, which is not a valid constant template argument"
//@ diag: 77:21 error "call to consteval function 'std::meta::reflect_constant<K>' is not a constant expression"
//@ diag: 77:21 note "the value refers to a string literal, which is not a valid constant template argument"
//@ diag: 78:16 error "constexpr variable 'n1' must be initialized by a constant expression"
//@ diag: 78:21 note "'NS' is not a structural type"
//@ diag: 78:21 error "call to consteval function 'std::meta::reflect_constant<NS>' is not a constant expression"
//@ diag: 78:21 note "'NS' is not a structural type"
//@ diag: 79:16 error "constexpr variable 'n2' must be initialized by a constant expression"
//@ diag: 79:21 note "'const char[4]' is not a structural type"
//@ diag: 79:21 error "call to consteval function 'std::meta::reflect_constant<char[4]>' is not a constant expression"
//@ diag: 79:21 note "'const char[4]' is not a structural type"
//@ diag: 81:16 error "constexpr variable 'n4' must be initialized by a constant expression"
//@ diag: 66:10 note "pointer to 'x' is not a constant expression"
//@ diag: 81:21 note "in call to 'local_object()'"
//@ diag: 65:7 note "declared here"
//@ diag: 81:21 error "call to consteval function 'local_object' is not a constant expression"
//@ diag: 66:10 note "pointer to 'x' is not a constant expression"
//@ diag: 81:21 note "in call to 'local_object()'"
//@ diag: 65:7 note "declared here"
//@ diag: 82:16 error "constexpr variable 'n5' must be initialized by a constant expression"
//@ diag: 70:10 note "pointer to 'x' is not a constant expression"
//@ diag: 82:21 note "in call to 'local_pointer()'"
//@ diag: 69:7 note "declared here"
//@ diag: 82:21 error "call to consteval function 'local_pointer' is not a constant expression"
//@ diag: 70:10 note "pointer to 'x' is not a constant expression"
//@ diag: 82:21 note "in call to 'local_pointer()'"
//@ diag: 69:7 note "declared here"
//@ diag: 83:16 error "constexpr variable 'n6' must be initialized by a constant expression"
//@ diag: 74:10 note "pointer to temporary is not a constant expression"
//@ diag: 83:21 note "in call to 'temp_object()'"
//@ diag: 73:18 note "temporary created here"
//@ diag: 83:21 error "call to consteval function 'temp_object' is not a constant expression"
//@ diag: 74:10 note "pointer to temporary is not a constant expression"
//@ diag: 83:21 note "in call to 'temp_object()'"
//@ diag: 73:18 note "temporary created here"
//@ diag: 84:16 error "constexpr variable 'n7' must be initialized by a constant expression"
//@ diag: 84:21 note "the value refers to a string literal, which is not a valid constant template argument"
//@ diag: 84:21 error "call to consteval function 'std::meta::reflect_constant<const char *>' is not a constant expression"
//@ diag: 84:21 note "the value refers to a string literal, which is not a valid constant template argument"
//@ meta-header: skip the real reflect_constant takes its argument by value, so a string literal decays
#include "kiln_meta.h"
using namespace std::meta;

struct K { char const* p; };
struct NS {
 private:
  int x = 0;

 public:
  constexpr NS() {}
};
consteval int cf() { return 1; }
consteval info local_object() {
  int x = 1;
  return reflect_object(x);
}
consteval info local_pointer() {
  int x = 1;
  return reflect_constant(&x);
}
consteval info temp_object() {
  const int& t = 3;
  return reflect_object(t);
}

constexpr info r4 = reflect_constant(K{"ebab"});
constexpr info n1 = reflect_constant(NS{});
constexpr info n2 = reflect_constant("abc");
constexpr info n3 = reflect_function(cf);
constexpr info n4 = local_object();
constexpr info n5 = local_pointer();
constexpr info n6 = temp_object();
constexpr info n7 = reflect_constant("abc" + 1);
