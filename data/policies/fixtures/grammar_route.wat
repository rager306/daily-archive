;; grammar_route fixture (ADR-074). Codes: 0=page_outline 1=xy_cut 2=grobid_structural
;; params: page_count, toc_bookmarks, text_layer_quality_milli (0..=1000)
(module
  (func (export "route") (param $pages i32) (param $bookmarks i32) (param $qmilli i32) (result i32)
    (if (result i32)
      (i32.and
        (i32.ge_u (local.get $pages) (i32.const 50))
        (i32.ge_u (local.get $bookmarks) (i32.const 1)))
      (then (i32.const 0))
      (else
        (if (result i32)
          (i32.lt_u (local.get $qmilli) (i32.const 400))
          (then (i32.const 2))
          (else (i32.const 1))))))
)
