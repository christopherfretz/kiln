// [dcl.link]/4: the linkage specifications that matter are the ones
// lexically enclosing a declaration. A namespace first defined inside
// `extern "C"` and reopened at file scope gives the declarations of the
// reopening no enclosing linkage specification, so redeclaring a function
// or function template there is not "a different language linkage"
// ([dcl.link]/6). The shape is libstdc++'s <type_traits> (`declval`
// declared, then defined) and <bits/move.h> (`swap`) reached after `std`
// was first opened by <cstdlib> under a user's `extern "C" { #include
// <stdlib.h> }` (as vendored headers in real code do).
//@ expect: run
//@ strict-diags
//@ exit: 9
extern "C" {
namespace n { extern "C++" { int first(int x) { return x; } } }
}
namespace n {
template <class T> T g();
template <class T> T g() { return T(3); }
int h(int);
int h(int x) { return x + 1; }
}
int main() { return n::g<int>() + n::h(4) + n::first(1); }
