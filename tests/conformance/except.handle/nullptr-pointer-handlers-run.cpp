// [except.handle]/3.4: a thrown `nullptr` (an exception object of type
// `std::nullptr_t`) is caught by a handler of type `cv T` or `const T&`
// where T is a pointer or pointer-to-member type, and the handler's object
// is a null pointer of that type; it is not caught by an `int`, `bool` or
// class handler (no conversion other than 3.4 applies; 3.1 needs the same
// type). The match is made by the personality routine through the type
// information of `decltype(nullptr)`.
//@ expect: run
//@ strict-diags
//@ stdout: ip cp fp mp mf ipr cpr fpr mpr mfr nt int-skipped bool-skipped class-skipped
#include <cstdio>
struct S {
  int m;
  void f() {}
};
struct FromNull {
  FromNull(decltype(nullptr)) {}
};
template <class H> bool caught_as() {
  try {
    throw nullptr;
  } catch (H h) {
    return h == nullptr;
  } catch (...) {
    return false;
  }
}
template <class H> bool skipped() {
  try {
    try {
      throw nullptr;
    } catch (H) {
      return false;
    }
  } catch (decltype(nullptr) p) {
    return p == nullptr;
  }
}
int main() {
  const char *out[] = {"ip", "cp", "fp", "mp", "mf", "ipr", "cpr", "fpr", "mpr", "mfr", "nt"};
  bool ok[] = {
      caught_as<int *>(),
      caught_as<const char *>(),
      caught_as<void (*)()>(),
      caught_as<int S::*>(),
      caught_as<void (S::*)()>(),
      caught_as<int *const &>(),
      caught_as<const char *const &>(),
      caught_as<void (*const &)()>(),
      caught_as<int S::*const &>(),
      caught_as<void (S::*const &)()>(),
      caught_as<const decltype(nullptr) &>(),
  };
  for (int i = 0; i < 11; ++i)
    if (ok[i])
      std::printf("%s ", out[i]);
  if (skipped<int>())
    std::printf("int-skipped ");
  if (skipped<bool>())
    std::printf("bool-skipped ");
  if (skipped<FromNull>())
    std::printf("class-skipped");
  std::printf("\n");
}
