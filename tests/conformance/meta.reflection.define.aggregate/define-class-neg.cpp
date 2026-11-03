// [meta.reflection.define.aggregate]/7.1-7.2 (P2996R13; spec/reflection.md
// §6.2): the class must be an incomplete, cv-unqualified class type not
// being defined: not a non-class, a complete class, a class defined once
// already by define_aggregate, nor a class whose member-specification
// encloses the call. A failure in a consteval block is not caught.
//@ expect: compile-fail
//@ args: -ferror-limit=0 -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 44:1 error "consteval block is not a constant expression"
//@ diag: 44:13 note "'^^int' is not a class type"
//@ diag: 44:1 note "in call to"
//@ diag: 45:1 error "consteval block is not a constant expression"
//@ diag: 45:13 note "'^^var' is not a class type"
//@ diag: 45:1 note "in call to"
//@ diag: 46:1 error "consteval block is not a constant expression"
//@ diag: 46:13 note "'^^En' is not a class type"
//@ diag: 46:1 note "in call to"
//@ diag: 47:1 error "consteval block is not a constant expression"
//@ diag: 47:13 note "'^^int *' is not a class type"
//@ diag: 47:1 note "in call to"
//@ diag: 49:1 error "consteval block is not a constant expression"
//@ diag: 49:13 note "'Done' is already complete"
//@ diag: 49:1 note "in call to"
//@ diag: 48:8 note "definition is here"
//@ diag: 52:1 error "consteval block is not a constant expression"
//@ diag: 52:13 note "'Twice' is already complete"
//@ diag: 52:1 note "in call to"
//@ diag: 50:8 note "definition is here"
//@ diag: 54:3 error "consteval block is not a constant expression"
//@ diag: 54:15 note "'Self' is being defined"
//@ diag: 54:3 note "in call to"
//@ diag: 57:1 error "consteval block is not a constant expression"
//@ diag: 57:13 note "'const CV' is a cv-qualified class type"
//@ diag: 57:1 note "in call to"
//@ diag: 59:1 error "consteval block is not a constant expression"
//@ diag: 61:5 note "'Done' is already complete"
//@ diag: 59:1 note "in call to"
//@ diag: 48:8 note "definition is here"
#include "kiln_meta.h"
using namespace std::meta;
int var;
enum En : int;
consteval { define_aggregate(^^int, {}); }
consteval { define_aggregate(^^var, {}); }
consteval { define_aggregate(^^En, {}); }
consteval { define_aggregate(^^int*, {}); }
struct Done {};
consteval { define_aggregate(^^Done, {}); }
struct Twice;
consteval { define_aggregate(^^Twice, {}); }
consteval { define_aggregate(^^Twice, {}); }
struct Self {
  consteval { define_aggregate(^^Self, {}); }
};
struct CV;
consteval { define_aggregate(^^const CV, {}); }
struct Caught;
consteval {
  try {
    define_aggregate(^^Done, {});
  } catch (...) {
  }
}
