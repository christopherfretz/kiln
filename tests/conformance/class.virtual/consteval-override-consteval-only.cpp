// [class.virtual]/18 as the working draft words it (P4101R1 removed
// P2996R13's consteval-only-type allowance): "A non-immediate virtual
// function shall not be overridden by an immediate virtual function." `M`
// holds a `std::meta::info` but its consteval `what` still may not override
// the constexpr one. Only an overrider declared in a system header is
// exempt (consteval-override-system-header.cpp). kiln only: GCC 16.1 keeps
// the P2996R13 rule and accepts `M` (docs/status/deviations.md).
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ diag: 22:25 error "consteval function 'what' cannot override a non-consteval function"
//@ diag: 18:33 note "overridden virtual function is here"
//@ diag: 24:38 error "consteval function 'what' cannot override a non-consteval function"
//@ diag: 18:33 note "overridden virtual function is here"
struct E {
  constexpr virtual ~E() = default;
  constexpr virtual const char* what() const noexcept { return "e"; }
};
struct M : E {
  decltype(^^int) from;
  consteval const char* what() const noexcept override { return "m"; }
};
struct N : E { consteval const char* what() const noexcept override { return "n"; } };
