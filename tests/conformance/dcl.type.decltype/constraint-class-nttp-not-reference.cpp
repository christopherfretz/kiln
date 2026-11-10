// [dcl.type.decltype]/1.3, [temp.param]/8, [temp.param]/6: `decltype` of a
// non-type parameter of class type declared `auto` names the template
// parameter object's class type, never a reference, also when the
// parameter is substituted from a type-constraint's arguments
// (`constexpr Of<sp> auto f()`), where the argument is still the written
// `sp` naming the variable rather than a template parameter object. Before
// WP-1217 kiln made `decltype(QS)` `const speed &` there and rejected the
// placeholder (mp-units' `QuantityOf<isq::speed> auto` return types).
//@ std: c++20
//@ expect: compile-ok
template <class T> concept not_ref = !__is_reference(T);
struct speed { int n = 1; };
inline constexpr speed sp{};
template <typename Q, auto QS> concept Of = not_ref<decltype(QS)>;
template <auto QS> constexpr bool viaconcept = Of<int, QS>;
template <auto QS> constexpr bool direct = __is_reference(decltype(QS));
template <const speed &R> constexpr bool byref = __is_reference(decltype(R));
template <typename Q, const speed &R> concept OfRef = __is_reference(decltype(R));
constexpr Of<sp> auto f(int x) { return x; }
constexpr OfRef<sp> auto g(int x) { return x; }
static_assert(viaconcept<sp>);
static_assert(!direct<sp>);
static_assert(byref<sp>);
static_assert(f(1) == 1 && g(2) == 2);
int main() { return f(0); }
