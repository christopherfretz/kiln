// [over.built]/16 equality candidates for `s != "x"` where `s` converts
// to `char*`: `operator!=(char*, char*)` needs the string literal to
// `char*` conversion ([depr.string], ill-formed since C++11 and ranked
// worst, clang's extension), and `operator!=(const char*, const char*)`
// adds a qualification conversion after the user-defined conversion.
// clang and GCC choose the second. kiln reports the call ambiguous,
// probably because it orders the first argument's two user-defined
// conversion sequences (identity before qualification,
// [over.ics.rank]/3.2.1) where clang finds them indistinguishable, their
// targets differing. Not yet reduced further. Found running GCC's C++
// testsuite.
//@ expect: compile-ok
//@ std: c++17
//@ xfail(kiln): WP-1473 remainder: built-in != between a class converting to char* and a string literal is reported ambiguous

struct S { operator char *(); };
bool f(S &s) { return s != "x"; }
