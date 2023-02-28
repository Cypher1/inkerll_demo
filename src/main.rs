use inkwell::OptimizationLevel;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::execution_engine::{ExecutionEngine, JitFunction};
use inkwell::module::Module;
use inkwell::targets::{CodeModel, RelocMode, FileType, Target, TargetTriple, InitializationConfig};
use std::error::Error;
use std::path::Path;
use std::process::Command;
use std::io::{stdout, stderr, Write};
use libc::SYS_exit;

// Look at 
// https://github.com/rust-lang/libc/blob/master/src/unix/linux_like/linux/gnu/b64/x86_64/not_x32.rs

/// Convenience type alias for the `sum` function.
///
/// Calling this is innately `unsafe` because there's no guarantee it doesn't
/// do `unsafe` operations internally.
type SumFunc = unsafe extern "C" fn(u64, u64, u64) -> u64;

struct CodeGen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    execution_engine: ExecutionEngine<'ctx>,
}

impl<'ctx> CodeGen<'ctx> {
    fn jit_compile_sum(&self) -> Option<JitFunction<SumFunc>> {
        let i64_type = self.context.i64_type();
        let fn_type = i64_type.fn_type(&[i64_type.into(), i64_type.into(), i64_type.into()], false);
        let function = self.module.add_function("sum", fn_type, None);
        let basic_block = self.context.append_basic_block(function, "entry");

        self.builder.position_at_end(basic_block);

        let x = function.get_nth_param(0)?.into_int_value();
        let y = function.get_nth_param(1)?.into_int_value();
        let z = function.get_nth_param(2)?.into_int_value();

        let sum = self.builder.build_int_add(x, y, "sum");
        let sum = self.builder.build_int_add(sum, z, "sum");

        self.builder.build_return(Some(&sum));

        unsafe { self.execution_engine.get_function("sum").ok() }
    }
}

fn write_machine() -> Option<()> {
    Target::initialize_x86(&InitializationConfig::default());

    let opt = OptimizationLevel::Default;
    let reloc = RelocMode::Default;
    let model = CodeModel::Default;
    // let path = Path::new("/tmp/some/path/main.asm");
    let path = Path::new("./main.elf");
    let target = Target::from_name("x86-64").unwrap();
    let target_machine = target.create_target_machine(
        &TargetTriple::create("x86_64-pc-linux-gnu"),
        "x86-64",
        "", // "+avx2",
        opt,
        reloc,
        model
    )
    .unwrap();

    let context = Context::create();
    let module = context.create_module("my_module");
    let void_type = context.void_type();
    let fn_type = void_type.fn_type(&[], false);

    module.add_function("my_fn", fn_type, None);

    let i8_type = context.i8_type();
    let i64_type = context.i64_type();
    let u64_type = context.i64_type(); // TODO: WAT
    let char_star_array_type = i64_type.into();
    // TODO: Learn from https://gota7.github.io/GotaGuide/ProgrammingLanguage/Llvm.html which does
    // a raw _start.
    let main_fn_type = i8_type.fn_type(&[i64_type.into(), char_star_array_type], false);
    let main = module.add_function("main", main_fn_type, None);
    {
        let basic_block = context.append_basic_block(main, "entry");
        let builder = context.create_builder();
        builder.position_at_end(basic_block);

        let argc = main.get_nth_param(0)?.into_int_value();
        builder.build_return(Some(&argc));
    }

    if false {
        let start_fn_type = void_type.fn_type(&[], false);
        let start = module.add_function("_start", start_fn_type, None);
        let basic_block = context.append_basic_block(start, "entry");
        let builder = context.create_builder();
        builder.position_at_end(basic_block);
        // https://stackoverflow.com/questions/16721164/x86-linux-assembler-get-program-parameters-from-start
        // let dummy_argc_val = i64_type.const_int(2, false);
        // let dummy_argv_val = i64_type.const_int(8000, false);
        // builder.build_call(main, &[dummy_argc_val.into(), dummy_argv_val.into()], "call_main");
        let sys_exit_val = u64_type.const_int(SYS_exit.try_into().unwrap(), false); // TODO: more wat
        let exit_status = i64_type.const_int(11, false);

        // SYSCALL
        let void_type = context.void_type();
        let fn_type = void_type.fn_type(&[], false);
        let syscall_wrapper = module.add_function("syscall_wrapper", fn_type, None);
        let basic_block = context.append_basic_block(syscall_wrapper, "entry");

        builder.position_at_end(basic_block);
        let asm_fn = context.i64_type().fn_type(&[context.i64_type().into(), context.i64_type().into()], false);
        let asm = context.create_inline_asm(
            /*ty*/ asm_fn,
            /*assembly*/ "mov eax,1\nint 0x80".to_string(),
            /*constraints*/ "=r,{rax},{rdi}".to_string(),
            /*sideeffects*/ true,
            /*alignstack*/ false,
            /*dialect*/ #[cfg(not(any(feature = "llvm4-0", feature = "llvm5-0", feature = "llvm6-0")))] None,
            #[cfg(not(any(
                feature = "llvm4-0",
                feature = "llvm5-0",
                feature = "llvm6-0",
                feature = "llvm7-0",
                feature = "llvm8-0",
                feature = "llvm9-0",
                feature = "llvm10-0",
                feature = "llvm11-0",
                feature = "llvm12-0"
            )))]
            /*can throw*/ false,
        );

        let params = &[sys_exit_val.into(), exit_status.into()];
        #[cfg(not(any(feature = "llvm15-0")))]
        {
            use inkwell::values::CallableValue;
            let callable_value = CallableValue::try_from(asm).expect("Couldn't convert...");
            builder.build_call(callable_value, params, "exit");
        }

        #[cfg(any(feature = "llvm15-0"))]
        builder.build_call(asm_fn, asm, params, "exit");

        builder.build_return(None);
    }

    // Can compile this to elf with
    // clang main.bc -o main.elf -target x86_64-pc-linux-gnu
    // assert!(module.write_bitcode_to_path(Path::new("./main.bc")));

    assert!(target_machine.write_to_file(&module, FileType::Object, &path).is_ok());

    let mut command = Command::new("clang");
    let cmd = command
        .arg(path)
        .arg("-o")
        .arg("main")
        .arg("-lc");

    let output = cmd.output().expect("failed to run clang");
    stdout().write_all(&output.stdout).unwrap();
    stderr().write_all(&output.stderr).unwrap();

    Some(())
}

fn main() -> Result<(), Box<dyn Error>> {
    /*
        let context = Context::create();
        let module = context.create_module("sum");
        let execution_engine = module.create_jit_execution_engine(OptimizationLevel::None)?;
        let codegen = CodeGen {
            context: &context,
            module,
            builder: context.create_builder(),
            execution_engine,
        };

        let sum = codegen.jit_compile_sum().ok_or("Unable to JIT compile `sum`")?;

        let x = 1u64;
        let y = 2u64;
        let z = 3u64;

        let res = unsafe {
            sum.call(x, y, z) // The unsafe thing is to call 'sum'.
        };

        println!("{} + {} + {} = {}", x, y, z, res);
        assert_eq!(res, x + y + z);

        dbg!(codegen.module.write_bitcode_to_path(Path::new("./a.bc")));
    */
    write_machine();

    Ok(())
}
