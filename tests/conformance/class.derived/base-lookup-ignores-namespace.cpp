// [class.derived.general]/2: lookup for a base class name considers only
// types, so namespace x is skipped and the enclosing struct x is found
// (clang 22, issue 16855). From clang's CXX/class.derived/p2.cpp. Found by
// the clang negative-test run (WP-912).
//@ std: c++20
//@ requires: clang>=22
//@ xfail(kiln): WP-912 follow-up, base-specifier lookup does not skip namespaces
//@ expect: compile-ok
struct x {};
namespace {
namespace x {
struct y : x {};
}
}
