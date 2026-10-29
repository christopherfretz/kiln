// Type trait operand checks (spec/sema-templates.md §9.1, "complete"): an
// incomplete operand where the trait needs a complete one, and the arity
// of clang's getTypeTraitArity.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:10 error "incomplete type 'I' used in type trait expression"
//@ diag: 21:8 note "forward declaration of 'I'"
//@ diag: 24:10 error "incomplete type 'I' used in type trait expression"
//@ diag: 21:8 note "forward declaration of 'I'"
//@ diag: 25:10 error "incomplete type 'const I' used in type trait expression"
//@ diag: 21:8 note "forward declaration of 'I'"
//@ diag: 26:26 error "incomplete type 'I' used in type trait expression"
//@ diag: 21:8 note "forward declaration of 'I'"
//@ diag: 27:10 error "type trait requires 2 arguments; have 1 argument"
//@ diag: 28:10 error "type trait requires 2 arguments; have 3 arguments"
//@ diag: 29:10 error "incomplete type 'I' used in type trait expression"
//@ diag: 21:8 note "forward declaration of 'I'"
//@ diag: 30:10 error "incomplete type 'I' used in type trait expression"
//@ diag: 21:8 note "forward declaration of 'I'"
//@ diag: 31:40 error "expected ','"
struct I;
struct S {};
bool a = __is_empty(I);
bool b = __is_trivially_copyable(I);
bool c = __is_polymorphic(const I);
bool d = __is_base_of(S, I);
bool e = __is_same(int);
bool f = __is_same(int, int, int);
bool g = __is_destructible(I);
bool h = __has_virtual_destructor(I);
unsigned long r = __array_extent(int[2]);
bool ok1 = __is_class(I) && !__is_union(I) && __is_base_of(I, I) && !__is_base_of(I, int);
bool ok2 = __is_destructible(I &) && !__is_destructible(void);
