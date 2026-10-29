#pragma pack(push, 1)
struct O {
  char c;
  struct In { char c; int i; } in;
#pragma pack(pop)
  struct Late { char c; int i; } late;
  int i;
};
