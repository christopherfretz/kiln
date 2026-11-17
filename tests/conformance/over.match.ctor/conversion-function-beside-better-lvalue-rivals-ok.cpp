// Siblings of conversion-function-beside-better-lvalue-cxx17-run.cpp with
// a rival conversion function whose object conversion is as good as that
// of 'operator B&()': an 'operator D()' yielding a derived class by
// value, and a conversion function template. clang 22 and GCC 16 accept
// both (clang through its CWG2327 candidates; GCC by its own tie-breaks
// when it binds the move constructor's 'B&&'); by the text they are
// ambiguous ([over.ics.rank]/3.3). Found running GCC's C++ testsuite.
//@ std: c++17
//@ expect: compile-ok
//@ xfail(kiln): WP-1625: GCC's tie-break between a better lvalue conversion and an equally good rival yielding a derived class by value, or a conversion function template
struct B { B(); B(const B &); B(B &&); };
struct D : B {};
struct ByValue { operator B() const; operator B &(); operator D(); };
struct Tmpl { operator B() const; operator B &(); template <class T> operator T(); };
void f(ByValue v, Tmpl t) {
  B a(v);
  B b(t);
}
