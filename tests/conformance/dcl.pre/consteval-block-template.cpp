// [dcl.pre]/P3289R1, [temp.inst]: a consteval block in a templated entity
// is evaluated once per instantiation (each specialization gets its own
// injected member; a block in a template definition is not evaluated
// until then), with the enclosing template's arguments.
//@ expect: compile-ok
//@ args: -Wno-kiln-extension -freflection -I tests/support
//@ skip(clang): clang 19 has no consteval blocks (P3289R1) or reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
int rt();
template <class T> struct Box {
  char tag;
  consteval { __kiln_define_member(^^Box, ^^T, "value"); }
  constexpr T get() const { return this->value; }
};
static_assert(sizeof(Box<char>) == 2);
static_assert(sizeof(Box<int>) == 2 * sizeof(int));
static_assert(Box<int>{'a', 7}.get() == 7);
static_assert(Box<short>{'b', 3}.get() == 3);

// Never evaluated: no specialization of Never is instantiated.
template <class T> struct Never {
  consteval { rt(); }
};
Never<int>* never_ptr = nullptr;

// A function template: one evaluation per specialization; an
// unevaluated specialization's block never runs.
template <class T> int f() {
  consteval { if (sizeof(T) > 8) rt(); }
  return sizeof(T);
}
int fi = f<int>() + f<char>();
template <class T> int g() {
  consteval { rt(); }
  return 0;
}

// A non-type template parameter and a member function template.
template <int N> struct Fixed {
  consteval { if (N < 0) rt(); }
  template <class U> static constexpr int scaled() {
    consteval { if (sizeof(U) * N > 64) rt(); }
    return sizeof(U) * N;
  }
};
static_assert(Fixed<2>::scaled<int>() == 2 * sizeof(int));

// A generic lambda's call operator template.
auto lam = [](auto x) {
  consteval { if (sizeof(x) > 8) rt(); }
  return x;
};
int li = lam(1) + lam('c');
