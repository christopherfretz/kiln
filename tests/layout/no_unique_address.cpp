struct E {};
struct F {};
struct N { [[no_unique_address]] E e; int i; };
struct M { [[no_unique_address]] E e1; [[no_unique_address]] E e2; char c; };
struct K { [[no_unique_address]] E e; [[no_unique_address]] F f; };
struct J { int i; [[no_unique_address]] E e; };
