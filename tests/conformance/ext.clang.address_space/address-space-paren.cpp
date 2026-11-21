//@ expect: compile-ok
//@ strict-diags
// An address space written right after a grouping `(` of a declarator
// qualifies the type at the parentheses, as clang applies the attributes
// of a paren chunk (WP-1812): `int (AS *p)` is a pointer to `AS int`, and
// `int *(AS *r)` a pointer to `AS` pointer to int.
#define AS(n) __attribute__((address_space(n)))
template <class A, class B> constexpr bool same_v = false;
template <class A> constexpr bool same_v<A, A> = true;

int (AS(1) *p);
int *(AS(1) *r);
int (*(AS(1) arr)[3]);
void (*AS(1) fp)();
void (*(AS(1) *hp))();
int (AS(1) q);
static_assert(same_v<decltype(p), AS(1) int *>);
static_assert(same_v<decltype(r), int *AS(1) *>);
static_assert(same_v<decltype(arr), int *AS(1)[3]>);
static_assert(same_v<decltype(fp), void (*AS(1))()>);
static_assert(same_v<decltype(hp), void (*AS(1) *)()>);
static_assert(same_v<decltype(q), AS(1) int>);
