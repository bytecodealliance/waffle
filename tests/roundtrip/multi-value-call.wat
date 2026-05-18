(module
  (func (export "f") (param i32 i32) (result i32 i32)
        local.get 0
        local.get 1
        call 1)

  (func (param i32 i32) (result i32 i32)
        local.get 0
        local.get 1))
