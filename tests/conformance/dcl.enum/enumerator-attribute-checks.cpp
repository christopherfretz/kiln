// [dcl.enum]/2, [dcl.attr.grammar]/5: the attribute-specifier-seq of an
// enumerator appertains to it, so its attributes are checked against an
// enumerator as any declaration's (clang `ActOnEnumConstant` ->
// `ProcessDeclAttributeList`): a subject that is not an enumerator is
// reported and the attribute dropped, in a dependent enumeration (a member
// of a template) too; the valid ones stay.
//@ verify

enum E {
  a [[deprecated("old")]],
  b __attribute__((deprecated)),
  c [[maybe_unused]],
  d [[nodiscard]],                     // expected-warning {{'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs}}
  e alignas(8),                        // expected-error {{'alignas' attribute only applies to variables, data members and tag types}}
  f __attribute__((enable_if(1, "x"))), // expected-warning {{'enable_if' attribute only applies to functions}}
  g [[noreturn]],                      // expected-error {{'noreturn' attribute only applies to functions}}
  h __attribute__((unavailable("gone"))), // expected-note {{'h' has been explicitly marked unavailable here}}
  i [[clang::annotate("x")]],
  j [[deprecated]] = 7,                // expected-note {{'j' has been explicitly marked deprecated here}}
};

int use_a() { return a; } // expected-warning {{'a' is deprecated: old}} expected-note@10 {{'a' has been explicitly marked deprecated here}}
int use_b() { return b; } // expected-warning {{'b' is deprecated}} expected-note@11 {{'b' has been explicitly marked deprecated here}}
int use_h() { return h; } // expected-error {{'h' is unavailable: gone}}
int use_j() { return j; } // expected-warning {{'j' is deprecated}}

template <class T> struct S {
  enum F {
    p [[nodiscard]],  // expected-warning {{'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs}}
    q [[noreturn]],   // expected-error {{'noreturn' attribute only applies to functions}}
    r [[deprecated]] = sizeof(T),
  };
};
S<int> s;
