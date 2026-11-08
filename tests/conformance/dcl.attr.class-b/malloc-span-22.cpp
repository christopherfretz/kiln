// clang 22's `malloc_span` (a function returning a two-field span-like
// struct): kiln does not know it, so it warns "unknown attribute" where
// clang checks its subject, while __has_attribute(malloc_span) is 1.
// WP-813.
//@ verify
//@ requires: clang>=22
//@ xfail(kiln): WP-813 follow-up, malloc_span (clang 22) is not implemented
struct sp { void *p; unsigned long n; };
__attribute__((malloc_span)) sp f1(unsigned long);
__attribute__((malloc_span)) int f2(); // expected-warning {{'malloc_span' attribute only applies to functions that return span-like structures}} expected-note {{returned type is not a struct type}}
__attribute__((malloc_span)) int v; // expected-warning {{'malloc_span' attribute only applies to functions}}
__attribute__((malloc_span(1))) sp f3(); // expected-error {{'malloc_span' attribute takes no arguments}}
struct bad { void *p; };
__attribute__((malloc_span)) bad f4(); // expected-warning {{'malloc_span' attribute only applies to functions that return span-like structures}} expected-note {{returned struct has 1 fields, expected 2}}
