// [expr.new]/19, [conv.ptr]/3: placement arguments are passed as in a
// function call, so a pointer to a derived class converts to a pointer to
// its base class parameter, including a base that is not the first one.
// Reduced from clang's `new (C, DC) ObjCIvarDecl(...)`, where `DC` is an
// `ObjCContainerDecl *` and `Decl::operator new` takes a `DeclContext *`.
//@ expect: run
//@ stdout: 1 1 1 2 1
#include <cstddef>
#include <cstdio>

struct Ctx {
  alignas(16) unsigned char buf[256];
  std::size_t used = 0;
};
struct Parent {
  int id;
};
Parent *last_parent;
struct Key {
  unsigned v;
};

struct Node {
  Parent *parent = nullptr;
  int tag = 0;
  void *operator new(std::size_t size, Ctx &c, Key, std::size_t extra = 0) {
    void *p = c.buf + c.used;
    c.used += size + extra;
    return p;
  }
  void *operator new(std::size_t size, Ctx &c, Parent *p, std::size_t extra = 0) {
    void *mem = c.buf + c.used;
    c.used += size + extra;
    last_parent = p;
    return mem;
  }
  void operator delete(void *) {}
  explicit Node(Parent *p) : parent(p) {}
};

struct Named {
  int pad[3];
};
struct Container : Named, Parent {};
struct Leaf : Node {
  using Node::Node;
};

int main() {
  Ctx c;
  Container k;
  k.id = 1;
  const Container *ck = &k;
  Leaf *a = new (c, &k) Leaf(&k);
  Node *b = new (c, const_cast<Container *>(ck), 8) Node(&k);
  bool adjusted = last_parent == static_cast<Parent *>(&k);
  Node *d = new (c, Key{3}) Node(nullptr);
  d->tag = 2;
  std::printf("%d %d %d %d %d\n", a->parent->id, b->parent == static_cast<Parent *>(&k), adjusted,
              d->tag,
              c.used >= 3 * sizeof(Node) + 8);
}
