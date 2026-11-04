// [temp.mem.class]/1, [temp.inst]/3, [class.mem]/5: a member class of a class
// template declared before its definition in the class (`struct P; struct P {
// ... };`) is instantiated as two redeclarations; a member function body and
// a nested member class definition instantiated later name the members of
// the defined one (WP-582). Before the fix the body `Entry e; ...` was
// dropped and `Header`'s `Entry*` / `Entry**` members got an error type
// (lowered as the invalid LLVM type `i0`).
//@ expect: run
//@ strict-diags
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
