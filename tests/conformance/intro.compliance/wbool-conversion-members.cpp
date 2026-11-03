// [intro.compliance.general]/2, [conv.bool]: bool-conversion holds
// pointer-bool-conversion and undefined-bool-conversion, and conversion
// holds bool-conversion (WP-408). Probe: clang++-19 and clang++-22
// -fsyntax-only -std=c++26 -Wno-everything -Wbool-conversion on this file
// give exactly the diagnostics below (-Wconversion gives the same).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-everything -Wbool-conversion
//@ diag: 16:30 warning "'this' pointer cannot be null in well-defined C++ code; pointer may be assumed to always convert to true"
//@ diag: 17:26 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; pointer may be assumed to always convert to true"
//@ diag: 18:10 warning "address of function 'f' will always evaluate to 'true'"
//@ diag: 18:10 note "prefix with the address-of operator to silence this warning"
//@ diag: 19:10 warning "address of array 'arr' will always evaluate to 'true'"
void f();
int arr[2];
struct S { bool g() { return this; } };
bool h(int &r) { return &r; }
bool a = f;
bool b = arr;
