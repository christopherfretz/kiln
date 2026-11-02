// [dcl.pre]/11 (P2741R3): a static_assert-message that is not a string
// literal must be an object whose `size()` converts to `std::size_t` and
// whose `data()` converts to `const char *` (checked whether or not the
// assertion holds); a failing assertion's message must be a constant
// expression. clang also evaluates the message of an assertion that holds
// (-Winvalid-static-assert-message, an error by default).
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 70:21 error "the message in a static assertion must be a string literal or an object with 'data()' and 'size()' member functions"
//@ diag: 71:22 error "the message in a static assertion must be a string literal or an object with 'data()' and 'size()' member functions"
//@ diag: 71:15 error "static assertion failed"
//@ diag: 72:22 error "the message object in this static assertion is missing a 'size()' member function"
//@ diag: 72:15 error "static assertion failed"
//@ diag: 73:22 error "the message object in this static assertion is missing a 'data()' member function"
//@ diag: 73:15 error "static assertion failed"
//@ diag: 74:22 error "the message object in this static assertion is missing 'data()' and 'size()' member functions"
//@ diag: 74:15 error "static assertion failed"
//@ diag: 75:21 error "value of type 'int' is not implicitly convertible to 'const char *'"
//@ diag: 75:21 error "the message in a static assertion must have a 'data()' member function returning an object convertible to 'const char *'"
//@ diag: 76:22 error "value of type 'const wchar_t *' is not implicitly convertible to 'const char *'"
//@ diag: 76:22 error "the message in a static assertion must have a 'data()' member function returning an object convertible to 'const char *'"
//@ diag: 76:15 error "static assertion failed"
//@ diag: 77:22 error "the message in a static assertion must be produced by a constant expression"
//@ diag: 77:22 note "non-constexpr function 'size' cannot be used in a constant expression"
//@ diag: 59:17 note "declared here"
//@ diag: 77:15 error "static assertion failed"
//@ diag: 78:22 error "the message in a static assertion must be produced by a constant expression"
//@ diag: 78:22 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 78:15 error "static assertion failed: ab<U+0000>"
//@ diag: 79:22 error "the message in a static assertion must be produced by a constant expression"
//@ diag: 79:22 note "read of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 79:15 error "static assertion failed"
//@ diag: 80:21 error "the message in this static assertion is not a constant expression [-Winvalid-static-assert-message]"
//@ diag: 80:21 note "non-constexpr function 'size' cannot be used in a constant expression"
//@ diag: 59:17 note "declared here"
//@ diag: 81:62 error "the message in a static assertion must be produced by a constant expression"
//@ diag: 82:14 note "in instantiation of template class 'Tm<NotConst>' requested here"
//@ diag: 81:62 note "non-constexpr function 'size' cannot be used in a constant expression"
//@ diag: 59:17 note "declared here"
//@ diag: 81:46 error "static assertion failed due to requirement 'sizeof(NotConst) == 0'"
//@ diag: 81:56 note "expression evaluates to '1 == 0'"
//@ diag: 81:62 error "the message object in this static assertion is missing 'data()' and 'size()' member functions"
//@ diag: 83:13 note "in instantiation of template class 'Tm<Neither>' requested here"
//@ diag: 81:46 error "static assertion failed due to requirement 'sizeof(Neither) == 0'"
//@ diag: 81:56 note "expression evaluates to '1 == 0'"
struct NoSize { constexpr const char* data() const { return "x"; } };
struct NoData { constexpr unsigned long size() const { return 1; } };
struct Neither {};
struct BadData {
  constexpr unsigned long size() const { return 1; }
  constexpr int data() const { return 0; }
};
struct WideData {
  constexpr unsigned long size() const { return 1; }
  constexpr const wchar_t* data() const { return L"x"; }
};
struct NotConst {
  unsigned long size() const { return 1; }
  constexpr const char* data() const { return "x"; }
};
struct PastEnd {
  constexpr unsigned long size() const { return 5; }
  constexpr const char* data() const { return "ab"; }
};
struct NullData {
  constexpr unsigned long size() const { return 2; }
  constexpr const char* data() const { return nullptr; }
};
static_assert(true, 42);
static_assert(false, ("parenthesized"));
static_assert(false, NoSize{});
static_assert(false, NoData{});
static_assert(false, Neither{});
static_assert(true, BadData{});
static_assert(false, WideData{});
static_assert(false, NotConst{});
static_assert(false, PastEnd{});
static_assert(false, NullData{});
static_assert(true, NotConst{});
template <class T> struct Tm { static_assert(sizeof(T) == 0, T{}); };
Tm<NotConst> tm1;
Tm<Neither> tm2;
