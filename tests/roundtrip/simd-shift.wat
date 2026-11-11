(module
  (func (param v128 i32) (result v128)
        (v128.not
          (i64x2.shr_u
            (i64x2.shr_s
              (i64x2.shl
                (i32x4.shr_u
                  (i32x4.shr_s
                    (i32x4.shl
                      (i16x8.shr_u
                        (i16x8.shr_s
                          (i16x8.shl (local.get 0) (local.get 1))
                          (local.get 1))
                        (local.get 1))
                      (local.get 1))
                    (local.get 1))
                  (local.get 1))
                (local.get 1))
              (local.get 1))
            (local.get 1)))))
