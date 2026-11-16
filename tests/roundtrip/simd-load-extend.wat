(module
  (memory 1)
  (func (param i32) (result v128)
        (v128.not (v128.load8x8_s (local.get 0)))))
