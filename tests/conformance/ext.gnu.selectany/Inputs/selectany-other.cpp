#include "selectany.h"
int *other_answer() { return &answer; }
const int *other_limit() { return &limit; }
int *other_table() { return table; }
int *other_dynamic() { return &dynamic; }
Counted *other_counted() { return &counted; }
int *other_member() { return &Holder::member; }
int *other_per_thread() { return &per_thread; }
