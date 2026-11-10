// The using family in a module interface, for image-using-run.cpp and
// image-using-hidden.cpp (spec/modules.md §11.11, stage S6):
// exported using-declarations (of functions, an overload set, an
// operator, classes), a using-enum-declaration, namespace aliases, a
// using-directive, and a non-exported using-declaration of a
// non-exported function.
export module imgu;
export namespace impl {
struct Box { int v; };
constexpr int get(Box b) { return b.v; }
constexpr int twice(int x) { return 2 * x; }
constexpr int twice(long x) { return 3 * (int)x; }
enum class Color { red = 1, green = 2, blue = 4 };
namespace deep {
constexpr int k = 5;
struct Tag {};
constexpr int probe(Tag) { return 11; }
} // namespace deep
struct Op { int v; };
constexpr Op operator+(Op a, Op b) { return {a.v + b.v}; }
} // namespace impl
namespace priv {
struct Raw { int v; };
constexpr int secret() { return 42; }
} // namespace priv
export namespace api {
using impl::Box;
using impl::get;
using impl::twice;
using impl::operator+;
using impl::Op;
using enum impl::Color;
namespace d = impl::deep;
using d::Tag;
using d::probe;
constexpr int viaalias() { return d::k; }
using namespace impl::deep;
constexpr int viadir() { return k + probe(Tag{}); }
} // namespace api
namespace api {
using priv::secret;
constexpr int peek() { return secret(); }
} // namespace api
export namespace api {
constexpr int peek2() { return peek(); }
constexpr priv::Raw make(int v) { return {v}; }
} // namespace api
export namespace ali = api;
export namespace ali2 = ali;
export constexpr int total() {
  return api::twice(1) + ali::twice(1L) + ali2::get({4}) + (int)api::green;
}
