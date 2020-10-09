// This struct is ABI-compatible with &[u8] in Rust.
typedef struct {
  char* data;
  size_t len;
} ByteSlice;

// ByteSlice read_from_pg(struct varlena* arg);
// text* is allocated via palloc, so its memory is managed by the DBMS. text is
// a UTF-8 interpretable buffer without a NULL at the end.
// struct varlena* copy_to_pg(ByteSlice s);
