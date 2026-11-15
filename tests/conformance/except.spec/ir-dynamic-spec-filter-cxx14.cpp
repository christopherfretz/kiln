// C++14 [except.spec]/9 and [except.unexpected], Itanium C++ ABI 2.5.3: a
// function defined with a dynamic exception specification gets landing
// pads with a `filter` clause listing the allowed type-infos (without
// reference and cv-qualifiers, `[0 x ptr] zeroinitializer` for `throw()`),
// after the catch clauses of an enclosing try and with `cleanup` when a
// local needs destroying; the filter's dispatch block calls
// `__cxa_call_unexpected` when the selector is negative and resumes
// otherwise. A specification on a declaration only does nothing.
//@ expect: ir
//@ min-oracle: 22
//@ std: c++14
//@ args: -w
struct A {};
struct D { ~D(); };
void g();
void f1() throw(int, const A&) { g(); }
void f2() throw() { g(); }
void f3() throw(int) { D d; g(); }
void f4() throw(A*) { try { g(); } catch (int) {} }
void f5() throw(int) try { g(); } catch (char) {}
void f6() throw(int);
void f7() { f6(); }
struct S { S() throw(int); ~S() throw(); };
S::S() throw(int) { g(); }
S::~S() throw() { g(); }
