//@ expect: compile-fail
//@ skip(clang): clang 19 does not implement P3394 annotations
//@ strict-diags
//@ diag: 28:3 error "an annotation cannot appertain to a using-directive"
//@ diag: 29:3 error "an annotation cannot appertain to an attribute-declaration"
//@ diag: 30:13 error "an annotation cannot appertain to a namespace alias"
//@ diag: 32:9 error "an annotation cannot appertain to a concept"
//@ diag: 33:13 error "an annotation cannot appertain to a template parameter"
//@ diag: 35:3 error "an annotation cannot appertain to a deduction guide"
//@ diag: 36:13 error "an annotation cannot appertain to a parameter of type 'void'"
//@ diag: 38:31 error "an annotation cannot appertain to a friend declaration that is not a definition"
//@ diag: 40:28 error "an annotation can appertain only to a declaration"
//@ diag: 41:20 error "an annotation cannot appertain to a lambda call operator"
//@ diag: 43:12 error "an annotation cannot appertain to a structured binding"
//@ diag: 45:5 error "an annotation can appertain only to a declaration"
//@ diag: 48:5 error "an annotation cannot appertain to a label"
//@ diag: 51:5 error "an annotation cannot appertain to a block-scope declaration whose target scope is a namespace"
//@ diag: 52:5 error "an annotation cannot appertain to a block-scope declaration whose target scope is a namespace"
//@ diag: 56:1 error "an attribute list cannot appear here"
//@ diag: 58:5 error "an annotation cannot appertain to an asm-declaration"
// [dcl.attr.annotation]/1: an annotation appertains only to a declaration
// of a type, type alias, variable, function, non-void function parameter,
// namespace, enumerator or non-static data member, not to a statement or a
// type, not when the declaration's host scope differs from its target
// scope (a block-scope function or `extern` declaration), and not to a
// non-defining friend declaration.
namespace m { int z; }
[[=1]] using namespace m;
[[=2]];
namespace [[=3]] alias = m;
template <class T> concept
    C [[=4]] = true;
template <[[=5]] int N> struct A { };
template <class T> struct B { B(T); };
[[=6]] B(int) -> B<int>;
void none([[=7]] void);
struct Friendly {
  friend void declared_only [[=8]] ();
};
int in_type = sizeof(int [[=9]]);
auto lambda = [] [[=10]] () { };
struct Pair { int x, y; };
auto [px [[=11]], py] = Pair{1, 2};
void statements() {
  [[=12]] return;
}
void labels() {
  [[=13]] done: ;
}
void block_scope() {
  [[=14]] extern int e;
  [[=15]] void local_function();
  [[=16]] int fine = 0;
  (void)fine;
}
[[=17]] asm("");
void block_asm() {
  [[=18]] asm("");
}
