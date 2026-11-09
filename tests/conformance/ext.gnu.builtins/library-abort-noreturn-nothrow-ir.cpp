// A user declaration of a library builtin gets the builtin's attributes
// when it is the first declaration, has C linkage, is at file scope and
// its type matches the builtin's ignoring the exception specification
// (clang's `AddKnownFunctionAttributes`, `hasSameFunctionTypeIgnoringExceptionSpec`).
// `abort` is `noreturn nothrow`: the noreturn bit is part of the type, so
// a first declaration with GNU `noreturn` matches and gets `nounwind`; the
// call is then a `call`, not an `invoke`, even with a cleanup pending.
//@ expect: ir
//@ min-oracle: 22
//@ std: c++20
extern "C" void abort() __attribute__((noreturn));

struct D { ~D(); };

void f(int x) {
  D d;
  if (x)
    abort();
}
