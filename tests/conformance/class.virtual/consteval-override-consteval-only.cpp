// [class.virtual]/18 as P2996R13 words it: a class with a consteval virtual
// function that overrides a non-consteval one shall have consteval-only type
// ([basic.types.general]). `M` holds a `std::meta::info`, so its consteval
// `what` may override the constexpr one (the shape of `std::meta::exception`
// in GCC 16.1's `<meta>`); `N` may not. Compared with GCC 16.1.
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 21:38 error "consteval function 'what' cannot override a non-consteval function"
//@ diag: 15:33 note "overridden virtual function is here"
struct E {
  constexpr virtual ~E() = default;
  constexpr virtual const char* what() const noexcept { return "e"; }
};
struct M : E {
  decltype(^^int) from;
  consteval const char* what() const noexcept override { return "m"; }
};
struct N : E { consteval const char* what() const noexcept override { return "n"; } };
