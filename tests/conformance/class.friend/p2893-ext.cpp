//@ std: c++23
//@ requires: clang>=22
//@ strict-diags
//@ diag: 9:43 warning "variadic 'friend' declarations are a C++2c extension"
//@ diag: 10:24 warning "variadic 'friend' declarations are a C++2c extension"
// [class.friend]/3 (P2893): before C++26 a friend pack expansion or a
// friend-type-specifier-list is accepted as an extension (clang 22
// `-Wc++26-extensions`), reported once per declaration at its `...` or `,`.
template <class... Ts> class C { friend Ts...; };
class D { friend C<int>, C<char>; };
