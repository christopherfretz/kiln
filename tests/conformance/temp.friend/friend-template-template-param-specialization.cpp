// [temp.friend]/1, [dcl.type.elab]: `friend class-key TT<args>;` names a
// specialization of a template template parameter; the elaborated
// specifier parses the template-id and befriends whatever specialization
// it substitutes to (libc++ <__node_handle>: `friend struct
// _MapOrSetSpecifics<_NodeType, __basic_node_handle>;`).
//@ expect: run
//@ strict-diags
template <class N, class H> struct Spec {
  int get(H& h) { return h.v; }
};
template <class N, template <class, class> class M>
class Handle : public M<N, Handle<N, M>> {
  friend struct M<N, Handle<N, M>>;
  int v = 4;
};
int main() {
  Handle<int, Spec> h;
  return h.get(h) == 4 ? 0 : 1;
}
