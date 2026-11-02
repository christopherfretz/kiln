// [depr.volatile.type]: the deprecation is C++20; C++17 accepts a
// volatile-qualified return type silently.
//@ std: c++17
//@ strict-diags
volatile int f();
const volatile int g();
int main() {}
