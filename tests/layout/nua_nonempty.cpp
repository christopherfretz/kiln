class P { int i; char c; };
struct Q { [[no_unique_address]] P p; char d; };
struct R { [[no_unique_address]] P p; [[no_unique_address]] P q; };
