// [namespace.qual]/2-3: qualified lookup through two using-directives that
// nominate namespaces declaring the same name finds both: ambiguous.
//@ verify
namespace A { int x; } // expected-note {{candidate found by name lookup is 'A::x'}}
namespace B { int x; } // expected-note {{candidate found by name lookup is 'B::x'}}
namespace C { using namespace A; using namespace B; }
int *p = &C::x; // expected-error {{reference to 'x' is ambiguous}}
