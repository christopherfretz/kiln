// GCC section: clang `handleSectionAttr` and its subject list
// (`Function, GlobalVar`): exactly one string literal argument, not on
// a variable with automatic storage duration or a type.
//@ verify

int a __attribute__((section(1))); // expected-error {{expected string literal as argument of 'section' attribute}}
int b __attribute__((section)); // expected-error {{'section' attribute takes one argument}}
int c __attribute__((section("x", "y"))); // expected-error {{'section' attribute takes one argument}}

void f() {
  int local __attribute__((section("s"))) = 0; // expected-error {{'section' attribute only applies to functions, global variables, Objective-C methods, and Objective-C properties}}
  static int ok __attribute__((section("s"))) = 0;
  (void)local;
  (void)ok;
}

struct __attribute__((section("s"))) T {}; // expected-error {{'section' attribute only applies to functions, global variables, Objective-C methods, and Objective-C properties}}
