// `void abort();` without `noreturn` does not match the library builtin's
// type (`noreturn` is part of the function type and clang's
// `hasSameFunctionTypeIgnoringExceptionSpec` compares it), so it gets no
// builtin attributes: the call may throw and is an `invoke` under a
// pending cleanup. The same holds for `[[noreturn]]`, a declaration
// attribute that does not change the type.
//@ expect: ir
//@ std: c++20
extern "C" void abort();
extern "C" [[noreturn]] void exit(int);

struct D { ~D(); };

void f(int x) {
  D d;
  if (x)
    abort();
  else
    exit(1);
}
