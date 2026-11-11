(module
  (func (param v128) (result i32)
        (i32.add
          (i32.add (i8x16.bitmask (local.get 0)) (i16x8.bitmask (local.get 0)))
          (i32.add (i32x4.bitmask (local.get 0)) (i64x2.bitmask (local.get 0))))))
