// C++98 to C++14 [except.spec] (C++14 [except.spec]/9-10) and
// [except.unexpected]: a dynamic exception specification filters at run
// time. A throw it does not allow calls the unexpected handler (also from a
// function-try-block's handler, a virtual function, a call through a
// pointer, a template whose list names its parameter, a constructor, a
// destructor, after the locals' cleanups); a derived class or a pointer
// conversion a handler would accept is allowed; what the handler throws
// propagates if allowed, becomes std::bad_exception if the list names it,
// and calls terminate otherwise (throw() included). kiln emitted no filter
// and the exceptions escaped (found running GCC's C++ testsuite). The body
// is Inputs/dynamic-spec-unexpected.h.
//@ expect: run
//@ std: c++98
//@ args: -w
//@ stdout: U f1 42
//@ stdout: f2 A
//@ stdout: f3 ptr
//@ stdout: U f4 bad
//@ stdout: U f5 42
//@ stdout: U f6 42
//@ stdout: U f7 42
//@ stdout: U f8 42
//@ stdout: U ctor 42
//@ stdout: U dtor 42
//@ stdout: ~D U f9 42
//@ stdout: U terminate
#include "Inputs/dynamic-spec-unexpected.h"
