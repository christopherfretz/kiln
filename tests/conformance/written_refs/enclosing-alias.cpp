// The written-reference table (spec/ast.md §18.3): an alias-declaration
// ([dcl.typedef]/2) holds the names of its type-id, as a typedef name
// does: at namespace, class and block scope, and in an alias template
// ([temp.alias]/1), whose pattern holds them. A typedef of a function
// type: its unnamed parameter holds the parameter's type, as in a
// function declaration ([dcl.fct]/3).
//@ written-refs: enclosing-alias.refs
namespace lib {
struct Shape {};
template <class T> struct Box {};
} // namespace lib

using Alias = lib::Shape;
typedef lib::Shape Named;
template <class T> using Boxed = lib::Box<T>;
typedef void Callback(lib::Shape, int);

struct Holder {
  using Member = lib::Box<lib::Shape>;
  typedef lib::Shape Nested;
};

int local() {
  using Local = lib::Shape;
  Local l;
  (void)l;
  return 0;
}
