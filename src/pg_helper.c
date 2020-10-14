#include <string.h>
#include "postgres.h"
#include "fmgr.h"
#include "pg_helper.h"

ByteSlice read_from_pg(struct varlena* arg) {
  ByteSlice s;
  s.len = VARSIZE(arg);
  s.data = VARDATA(arg);
  return s;
}

struct varlena* palloc_varlena(size_t sz) {
  struct varlena* data = (struct varlena *) palloc(VARHDRSZ + sz);
  SET_VARSIZE(data, VARHDRSZ + sz);
  return data;
}

// Here is how to deliver struct varlena data to PostgreSQL. char* is not necessarily
// nul-terminated.
struct varlena* copy_to_pg(ByteSlice s) {
  struct varlena *dst = palloc_varlena(s.len);
  memcpy((void*) VARDATA(dst), (void*) s.data, s.len);
  return dst;
}

char* bytes_ptr(struct varlena* t) {
  return (char*) VARDATA(t);
}

size_t bytes_len(struct varlena* t) {
  return VARSIZE(t) - VARHDRSZ;
}

void report(int kind, char* msg) {
  ereport(kind, (errmsg(msg)));
}

