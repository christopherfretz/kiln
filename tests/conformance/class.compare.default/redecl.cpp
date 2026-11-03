// [dcl.constexpr]/1, [except.spec]/5, [class.compare.default]: the
// declarations of a defaulted comparison agree on constexpr, both orders;
// a friend defaulted in its class has the implicit exception
// specification computed at the end of the class ([except.spec]/9,
// non-throwing here), which a later redeclaration must repeat before the
// constexpr check is reached (clang `CheckEquivalentExceptionSpec`
// resolves it in `MergeFunctionDecl`). An in-class defaulted comparison
// that can be constexpr is implicitly constexpr ([dcl.fct.def.default]/3).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:19 error "constexpr declaration of 'operator==' follows non-constexpr declaration"
//@ diag: 23:24 note "previous declaration is here"
//@ diag: 26:9 error "non-constexpr declaration of 'operator==' follows constexpr declaration"
//@ diag: 25:34 note "previous declaration is here"
//@ diag: 28:16 error "constexpr declaration of 'operator==' follows non-constexpr declaration"
//@ diag: 27:31 note "previous declaration is here"
//@ diag: 30:6 error "'operator==' is missing exception specification 'noexcept'"
//@ diag: 29:41 note "previous declaration is here"
//@ diag: 32:16 error "'operator==' is missing exception specification 'noexcept'"
//@ diag: 31:41 note "previous declaration is here"
//@ diag: 34:6 error "non-constexpr declaration of 'operator==' follows constexpr declaration"
//@ diag: 33:31 note "previous declaration is here"
struct A { int n; bool operator==(const A&) const; };
constexpr bool A::operator==(const A&) const = default;
struct B { int n; constexpr bool operator==(const B&) const; };
bool B::operator==(const B&) const = default;
struct C { int n; friend bool operator==(const C&, const C&); };
constexpr bool operator==(const C&, const C&) = default;
struct D { int n; friend constexpr bool operator==(const D&, const D&) = default; };
bool operator==(const D&, const D&);
struct E { int n; friend constexpr bool operator==(const E&, const E&) = default; };
constexpr bool operator==(const E&, const E&);
struct F { int n; friend bool operator==(const F&, const F&) = default; };
bool operator==(const F&, const F&) noexcept;
