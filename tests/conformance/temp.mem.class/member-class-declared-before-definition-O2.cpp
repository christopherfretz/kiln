// [temp.mem.class]/1, [temp.inst]/3: member-class-declared-before-definition.cpp
// at -O2 (WP-582): the member function and nested member class instantiated
// from a member class declared before its definition fold to the same result.
//@ expect: run
//@ strict-diags
//@ args: -O2
template <class T>
struct Queue {
  struct Producer;
  enum class Mode : int;
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
    int sum() {
      Entry e[2] = {{3, nullptr}, {4, nullptr}};
      Entry* ix[2] = {&e[1], &e[0]};
      Header h{2, e, ix, nullptr};
      int s = 0;
      for (unsigned long i = 0; i != h.capacity; ++i) s += h.entries[i].key * 10 + h.index[i]->key;
      return sizeof(Header) == sizeof(unsigned long) + 3 * sizeof(void*) ? s : -1;
    }
  };
  enum class Mode : int { a, b = 5 };
  int mode() const { return static_cast<int>(Mode::b); }
};
// Only the nested class's layout (no member function body): `Ring<long>`
// completes `Slot` through its data member, then `Header` is completed.
template <class T>
struct Ring {
  struct Slot;
  struct Slot {
    struct Entry {
      T key;
    };
    struct Header {
      unsigned long capacity;
      Entry* entries;
      Entry** index;
      Header* prev;
    };
  };
  Slot s;
};
static_assert(sizeof(Ring<long>::Slot::Header) == sizeof(unsigned long) + 3 * sizeof(void*));
Ring<long>::Slot::Header ring_header;
int main() {
  Queue<int> q;
  Queue<int>::Producer p;
  // (3*10 + 4) + (4*10 + 3) = 77
  return p.sum() == 77 && q.mode() == 5 && ring_header.entries == nullptr ? 0 : 1;
}
