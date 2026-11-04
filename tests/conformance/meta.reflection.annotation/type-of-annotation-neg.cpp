// [meta.reflection.queries] (P3394R4): type_of(r) needs r to have a
// type (has-type); an annotation does, but a class, a namespace and a
// type do not, so type_of throws std::meta::exception for those as
// before.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:15 note "'^^C' has no type"
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 25:15 note "'^^ns' has no type"
//@ diag: 26:15 error "static assertion expression is not an integral constant expression"
//@ diag: 26:15 note "'^^int' has no type"
//@ diag: 27:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:15 note "'^^const skip_t' has no type"
#include <meta>
using namespace std::meta;
struct skip_t {};
namespace ns {}
struct [[=skip_t{}]] C {};
static_assert(type_of(annotations_of(^^C)[0]) == ^^const skip_t);
static_assert(type_of(^^C) == ^^C);
static_assert(type_of(^^ns) == ^^int);
static_assert(type_of(^^int) == ^^int);
static_assert(type_of(type_of(annotations_of(^^C)[0])) == ^^int);
