// The written-reference table (spec/ast.md §18.4), role `base`: the
// class-or-decltype of each base-specifier ([class.derived.general]),
// plain, qualified, a template-id (role `base` on the template name, its
// arguments their own entries), a typedef, a template parameter, a
// dependent qualified base, a pack expansion and a base found through a
// using-declaration.
//@ written-refs: base-specifiers.refs
namespace lib {
struct Root {};
struct Mixin {};
template <class T> struct Wrap { T w; };
typedef Mixin MixinAlias;
} // namespace lib

using lib::Root;
using lib::Wrap;

struct Plain : Root {};
struct Qualified : lib::Root, virtual public lib::Mixin {};
struct FromTemplate : lib::Wrap<lib::Root> {};
struct FromTypedef : private lib::MixinAlias {};
struct FromUsingTemplate : Wrap<int> {};

template <class T> struct FromParam : T {};
template <class T> struct FromDependent : T::base_type, lib::Wrap<T> {};
template <class... Ts> struct FromPack : Ts... {};

FromParam<Root> fp;
FromPack<lib::Root, lib::Mixin> pk;
