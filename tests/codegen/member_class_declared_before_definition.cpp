// [temp.mem.class]/1, [temp.inst]/3 (WP-582): a member class of a class
// template declared before its definition (`struct Producer; struct Producer
// { ... };`). The instantiated nested `Header`'s `Entry*`, `Entry**` and
// `Header*` members are `ptr` (`%"struct.Queue<int>::Producer::Header" =
// type { i64, ptr, ptr, ptr }`), never the invalid `i0` an error-typed
// member lowered to, and the member function body that names `Entry` and
// `Header` is emitted rather than dropped.
template <class T>
struct Queue {
  struct Producer;
  struct Producer {
    struct Entry {
      T key;
      Entry* next;
    };
    struct Header {
      unsigned long capacity;
      Entry* entries;
      Entry** index;
      Header* prev;
    };
    Header* head = nullptr;
    unsigned long link(Header* h, Entry* e, Entry** ix) {
      h->capacity = sizeof(Header) + sizeof(Entry);
      h->entries = e;
      h->index = ix;
      h->prev = head;
      ix[0] = e;
      head = h;
      return h->capacity + static_cast<unsigned long>(h->entries->key);
    }
  };
  Producer p;
};
unsigned long use(Queue<int>& q, Queue<int>::Producer::Header* h,
                  Queue<int>::Producer::Entry* e, Queue<int>::Producer::Entry** ix) {
  return q.p.link(h, e, ix);
}
