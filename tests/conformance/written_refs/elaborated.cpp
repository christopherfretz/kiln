// The written-reference table (spec/ast.md §18.4, stage 3), role
// `elaborated`: an elaborated-type-specifier that declares
// ([dcl.type.elab]/2) and a friend declaration ([class.friend]) name
// the entity they redeclare (its first declaration). A first
// declaration's name (`struct Fresh;`, `friend class Unseen;`) is the
// entity's own and has no entry; one that refers without declaring
// (`struct Node *p;`) is a `type` entry (stage 2). A friend of a
// dependent qualifier is found at instantiation (`dep`).
//@ written-refs: elaborated.refs
struct Node;
struct Node {
  Node *next;
};
struct Node;
struct Fresh;
struct Node *head;
enum class Mode : int;
enum class Mode : int { fast };
enum class Mode : int;

namespace net {
struct Socket;
void close(Socket &);
int open(int);
} // namespace net

class Peer;
void connect(Peer &);
class Registry {
  friend class Peer;
  friend class Unseen;
  friend void connect(Peer &);
  friend struct net::Socket;
  friend void net::close(net::Socket &);
  friend int net::open(int);
};

template <class T> class Holder {
  friend class T::Key;
  friend T;
};
