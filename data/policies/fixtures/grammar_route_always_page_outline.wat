;; hot-swap fixture: always page_outline (code 0)
(module
  (func (export "route") (param i32 i32 i32) (result i32)
    (i32.const 0))
)
