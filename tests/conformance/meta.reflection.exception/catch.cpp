// [meta.reflection.exception] (P3560R2), [expr.const]/10 (P3068R6): a
// reflection function that fails inside a try block of the evaluation
// throws std::meta::exception, constructed from the failure's message, a
// reflection of the function (from()) and the call's location (where()).
// It is caught by type, as a std::exception, or by catch (...); a copy
// owns its message and survives the original; it can be rethrown.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ xfail(gcc): what() is kiln's message wording, which GCC 16.1's library does not share
#include "kiln_meta.h"
using namespace std::meta;

struct S {};

consteval bool by_type() {
  try {
    (void)identifier_of(^^int);
  } catch (const std::meta::exception& e) {
    return std::string_view(e.what()) == "'^^int' has no identifier" &&
           e.u8what() == u8"'^^int' has no identifier" && e.from() == ^^identifier_of;
  }
  return false;
}
static_assert(by_type());

consteval unsigned where_line() {
  try {
    (void)u8identifier_of(^^S*);  // line 30
  } catch (std::meta::exception e) {
    return e.from() == ^^u8identifier_of ? e.where().line() : 0;
  }
  return 0;
}
static_assert(where_line() == 30);

consteval bool as_base() {
  try {
    (void)identifier_of(^^const S);
  } catch (const std::exception& e) {
    return e.what()[0] == '\'';
  }
  return false;
}
static_assert(as_base());

consteval bool ellipsis() {
  try {
    (void)identifier_of(^^int);
  } catch (...) {
    return true;
  }
  return false;
}
static_assert(ellipsis());

// Thrown from a callee, caught in the caller; the inner unmatched handler
// is skipped.
consteval std::string_view inner() {
  try {
    return identifier_of(^^int);
  } catch (int) {
    return "int";
  }
}
consteval bool from_callee() {
  try {
    (void)inner();
  } catch (const std::meta::exception& e) {
    return e.from() == ^^identifier_of;
  }
  return false;
}
static_assert(from_callee());

// A copy owns its message; rethrow keeps the object.
consteval bool copy_and_rethrow() {
  std::meta::exception* keep = nullptr;
  try {
    try {
      (void)identifier_of(^^int);
    } catch (const std::meta::exception& e) {
      keep = new std::meta::exception(e);
      throw;
    }
  } catch (const std::meta::exception& e) {
    bool same = std::string_view(e.what()) == keep->what();
    delete keep;
    return same;
  }
  return false;
}
static_assert(copy_and_rethrow());

// No try block: a successful call throws nothing.
consteval bool no_failure() {
  try {
    return identifier_of(^^S) == "S";
  } catch (...) {
    return false;
  }
}
static_assert(no_failure());

int main() {}
