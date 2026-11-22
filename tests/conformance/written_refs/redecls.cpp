// The written-reference table (spec/ast.md §18.4, stage 3), role
// `redecl`: the declared name of a declaration with a qualified
// declarator-id or class-head-name ([dcl.meaning.general]/2,
// [class.pre]/3) names the declaration it redeclares (its first), after
// the qualifier's `nns` entries; both are enclosed by the declaration
// (§18.3). A constructor's, a destructor's (`~[T]`) and an operator's
// name too. An unqualified first declaration (`helper`, `Box`) is the
// entity's own name and has no entry.
//@ written-refs: redecls.refs
namespace app {
namespace core {
void run();
void run(int);
extern int level;
struct Engine {
  Engine();
  ~Engine();
  static int instances;
  int speed() const;
  Engine &operator+=(int);
  struct Part;
  template <class T> T convert() const;
};
} // namespace core
} // namespace app

void app::core::run(int) {}
int app::core::level = 3;
int app::core::Engine::instances = 0;
app::core::Engine::Engine() {}
app::core::Engine::~Engine() {}
int app::core::Engine::speed() const { return instances; }
app::core::Engine &app::core::Engine::operator+=(int) { return *this; }
struct app::core::Engine::Part {};
template <class T> T app::core::Engine::convert() const { return T(); }

template <class T> struct Box {
  void put(T);
  static T empty;
};
template <class T> void Box<T>::put(T) {}
template <class T> T Box<T>::empty = T();

void helper() {}
