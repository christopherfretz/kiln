// [basic.scope.block], [cpp.replace]: -Wshadow is one of the few warnings
// clang marks SuppressInSystemMacro (spec/diag.md §5.4): a declaration
// spelled in the body of a macro from a system header does not warn, even
// though the macro is expanded in a user file, and is not checked for
// -Wshadow-uncaptured-local either. A name passed as a macro argument
// from the user file still warns, and so does a user macro; a name pasted
// with ## inside the system macro counts as spelled in it.
//@ verify
//@ args: -Wshadow-all -isystem %{test_dir}/Inputs
#include <shadow_system_macros.h>
#define USER_INNER(x) __extension__({ int __u = (x); __u; })
#define USER_OUTER(x) __extension__({ int __u = USER_INNER(x); __u; })
#define USER_LAMBDA_SHADOW() ([] { int __loc = 1; return __loc; }())

int f(int v) { // expected-note {{previous declaration is here}}
  int a = SYS_OUTER(v);
  int b = USER_OUTER(v); // expected-warning {{declaration shadows a local variable}} expected-note {{previous declaration is here}}
  { SYS_DECLARE(v); } // expected-warning {{declaration shadows a local variable}}
  int ab = 0;
  { SYS_PASTE_DECL(a, b); }
  (void)ab;
  int __loc = 0; // expected-note {{previous declaration is here}}
  (void)__loc;
  return a + b + SYS_LAMBDA_SHADOW() + USER_LAMBDA_SHADOW(); // expected-warning {{declaration shadows a local variable}}
}
