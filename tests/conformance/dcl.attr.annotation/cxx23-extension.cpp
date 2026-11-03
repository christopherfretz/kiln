//@ expect: compile-ok
//@ skip(clang): clang 19 does not implement P3394 annotations
//@ std: c++23
//@ strict-diags
//@ diag: 8:3 warning "annotations are a C++26 extension"
// [dcl.attr.grammar]: annotations are C++26; earlier modes accept them as
// an extension with a warning (once per attribute-specifier).
[[=1]] int x;
int main() { return x; }
