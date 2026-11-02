// [temp.deduct.general]/8: a missing `type` in `enable_if` is a substitution
// failure; clang 19 words the deduction-failure note after the condition
// (spec/sema-templates.md §5.7.1): "requirement 'C' was not satisfied" with
// the first false operand of a `&&` chain, or "disabled by 'enable_if'" when
// the condition is already a value. A viable sibling is chosen silently.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:12 error "no matching function for call to 'f1'"
//@ diag: 18:86 note "candidate template ignored: requirement 'is_ptr<int>::value' was not satisfied [with P = int]"
//@ diag: 24:19 error "no matching function for call to 'f2'"
//@ diag: 19:72 note "candidate template ignored: requirement 'false || is_ptr<int>::value' was not satisfied [with P = int]"
//@ diag: 24:26 error "no matching function for call to 'f3'"
//@ diag: 20:66 note "candidate template ignored: disabled by 'enable_if' [with P = int, B = false]"
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { typedef T type; };
template <class T> struct is_ptr { static constexpr bool value = false; };
template <class T> struct is_ptr<T*> { static constexpr bool value = true; };
template <class P> typename enable_if<is_ptr<P>::value && sizeof(P) == 8, int>::type f1(P);
template <class P> typename enable_if<false || is_ptr<P>::value>::type f2(P);
template <class P, bool B = is_ptr<P>::value> typename enable_if<B, int>::type f3(P);
template <class P> typename enable_if<!is_ptr<P>::value, long>::type f4(P);
template <class P> typename enable_if<is_ptr<P>::value, int>::type f4(P);
static_assert(sizeof(f4(1)) == sizeof(long));
void g() { f1(1); f2(1); f3(1); }
