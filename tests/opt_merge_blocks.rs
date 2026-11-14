use waffle::{Block, BlockTarget, FunctionBody, Operator, OptOptions, Terminator, Type};
use Type::I32;

fn block_target(block: Block) -> BlockTarget {
    BlockTarget {
        block,
        args: Vec::new(),
    }
}

fn make_br_br_ret(const_bb: usize, iadd_bb: usize) -> FunctionBody {
    let mut body = FunctionBody::default();
    let b0 = body.add_block();
    let b1 = body.add_block();
    let b2 = body.add_block();

    let v0 = body.add_blockparam(b0, I32);
    let bbs = [b0, b1, b2];
    let v1 = body.add_op(bbs[const_bb], Operator::I32Const { value: 42 }, &[], &[I32]);
    let v2 = body.add_op(bbs[iadd_bb], Operator::I32Add, &[v0, v1], &[I32]);

    body.entry = b0;
    body.set_terminator(
        b0,
        Terminator::Br {
            target: block_target(b1),
        },
    );
    body.set_terminator(
        b1,
        Terminator::Br {
            target: block_target(b2),
        },
    );
    body.set_terminator(b2, Terminator::Return { values: vec![v2] });
    body.recompute_edges();
    body.validate().unwrap();
    body
}

#[test]
fn br_br_ret_with_inst() {
    let mut body = make_br_br_ret(1, 2);
    expect_test::expect![[r#"
        function() ->  {
          block0(v0: i32): # 
            # preds: 
            # succs: block1 ()
            br block1()
          block1(): # 
            # preds: block0 ()
            # succs: block2 ()
            v1 = i32const<42>  # i32  
            br block2()
          block2(): # 
            # preds: block1 ()
            # succs: 
            v2 = i32add v0, v1 # i32  
            return v2
        }
    "#]]
    .assert_eq(&body.display("", None).to_string());

    body.optimize(&OptOptions::default());
    expect_test::expect![[r#"
        function() ->  {
          block0(v0: i32): # 
            # preds: 
            # succs: block1 ()
            br block1()
          block1(): # 
            # preds: block0 ()
            # succs: block2 ()
            v1 = i32const<42>  # i32  
            br block2()
          block2(): # 
            # preds: block1 ()
            # succs: 
            v2 = i32add v0, v1 # i32  
            return v2
        }
    "#]]
    .assert_eq(&body.display("", None).to_string());
}

#[test]
fn br_br_ret_no_inst() {
    let mut body = make_br_br_ret(2, 2);
    expect_test::expect![[r#"
        function() ->  {
          block0(v0: i32): # 
            # preds: 
            # succs: block1 ()
            br block1()
          block1(): # 
            # preds: block0 ()
            # succs: block2 ()
            br block2()
          block2(): # 
            # preds: block1 ()
            # succs: 
            v1 = i32const<42>  # i32  
            v2 = i32add v0, v1 # i32  
            return v2
        }
    "#]]
    .assert_eq(&body.display("", None).to_string());

    body.optimize(&OptOptions::default());
    expect_test::expect![[r#"
        function() ->  {
          block0(v0: i32): # 
            # preds: 
            # succs: block2 ()
            br block2()
          block1(): # 
            # preds: 
            # succs: block2 ()
            br block2()
          block2(): # 
            # preds: block0 (), block1 ()
            # succs: 
            v1 = i32const<42>  # i32  
            v2 = i32add v0, v1 # i32  
            return v2
        }
    "#]]
    .assert_eq(&body.display("", None).to_string());
}

#[test]
fn condbr_br_ret() {
    let mut body = FunctionBody::default();
    let b0 = body.add_block();
    let b1 = body.add_block();
    let b2 = body.add_block();
    let b3 = body.add_block();

    let v0 = body.add_blockparam(b0, I32);
    let v1 = body.add_blockparam(b0, I32);

    body.entry = b0;
    body.set_terminator(
        b0,
        Terminator::CondBr {
            cond: v0,
            if_true: block_target(b1),
            if_false: block_target(b3),
        },
    );
    body.set_terminator(
        b1,
        Terminator::Br {
            target: block_target(b2),
        },
    );
    body.set_terminator(b2, Terminator::Return { values: vec![v0] });
    body.set_terminator(b3, Terminator::Return { values: vec![v1] });
    body.recompute_edges();
    body.validate().unwrap();

    expect_test::expect![[r#"
        function() ->  {
          block0(v0: i32, v1: i32): # 
            # preds: 
            # succs: block1 (), block3 ()
            if v0, block1(), block3()
          block1(): # 
            # preds: block0 ()
            # succs: block2 ()
            br block2()
          block2(): # 
            # preds: block1 ()
            # succs: 
            return v0
          block3(): # 
            # preds: block0 ()
            # succs: 
            return v1
        }
    "#]]
    .assert_eq(&body.display("", None).to_string());

    body.optimize(&OptOptions::default());
    expect_test::expect![[r#"
        function() ->  {
          block0(v0: i32, v1: i32): # 
            # preds: 
            # succs: block2 (), block3 ()
            if v0, block2(), block3()
          block1(): # 
            # preds: 
            # succs: block2 ()
            br block2()
          block2(): # 
            # preds: block0 (), block1 ()
            # succs: 
            return v0
          block3(): # 
            # preds: block0 ()
            # succs: 
            return v1
        }
    "#]]
    .assert_eq(&body.display("", None).to_string());
}
