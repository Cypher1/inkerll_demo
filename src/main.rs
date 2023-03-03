use inkwell::types::PointerType;
use inkwell::{OptimizationLevel, AddressSpace};
use inkwell::builder::Builder;
use inkwell::context::Context;
// use inkwell::execution_engine::{ExecutionEngine, JitFunction};
use inkwell::module::{Module, Linkage};
use inkwell::targets::{CodeModel, RelocMode, FileType, Target, InitializationConfig, TargetMachine};
use inkwell::values::{FunctionValue, PointerValue, BasicMetadataValueEnum};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::process::Command;
use std::io::{stdout, stderr, Write};
// use libc::SYS_exit;

// Look at 
// https://github.com/rust-lang/libc/blob/master/src/unix/linux_like/linux/gnu/b64/x86_64/not_x32.rs

/// Convenience type alias for the `sum` function.
///
/// Calling this is innately `unsafe` because there's no guarantee it doesn't
/// do `unsafe` operations internally.
// type SumFunc = unsafe extern "C" fn(u64, u64, u64) -> u64;

struct CodeGen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    // execution_engine: ExecutionEngine<'ctx>,
    strings: HashMap<String, PointerValue<'ctx>>,
}

impl<'ctx> CodeGen<'ctx> {
    /*
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
    */

    pub fn char_ptr_type(&mut self) -> PointerType<'ctx> {
        self.context.i8_type().ptr_type(AddressSpace::default())
    }

    /*
    pub fn global_string(&mut self, _value: &str) -> PointerValue<'ctx> {
        let value = "HELLO WORLD";
        self.strings.get(value).copied().unwrap_or_else(|| {
            println!("HERE3");
            dbg!(value);
            dbg!(&self.builder);
            // let ptr_value = self.builder.build_global_string_ptr(value, "global_string");
            // dbg!(ptr_value);
            let ptr_value = self.context.const_string(value.as_bytes(), false);
            let ptr = ptr_value.as_pointer_value();
            self.strings.insert(value.to_string(), ptr);
            println!("HERE4");
            ptr
        })
    }*/

    pub fn printf(&mut self, fmt: &str, args: &[BasicMetadataValueEnum<'ctx>]) {
        let printf = self.get_printf();
        println!("HERE1");
        let fmt_str = self.context.const_string(fmt.as_bytes(), false);
        // let fmt_str = self.global_string(fmt);
        println!("HERE2");
        let mut arg_array: Vec<BasicMetadataValueEnum<'ctx>> = vec![fmt_str.into()];
        println!("HERE3");
        arg_array.extend_from_slice(&args[..]);
        println!("HERE4");
        self.builder.build_call(printf, &arg_array[..], "_call_printf");
        println!("HERE5");
    }

    pub fn get_printf(&mut self) -> FunctionValue<'ctx> {
        let name = "printf";
        self.module.get_function(name).unwrap_or_else(|| {
            let printf_type = self.context.i32_type().fn_type(&[self.char_ptr_type().into()], true);
            self.module.add_function(name, printf_type, Some(Linkage::External))
        })
    }

    fn write_machine(&mut self) -> Option<()> {
        Target::initialize_x86(&InitializationConfig::default());
        // Target::initialize_all(&InitializationConfig::default());
        let target_triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&target_triple).unwrap();
        let reloc = RelocMode::Default;
        let model = CodeModel::Default;
        let opt = OptimizationLevel::Default;
        let target_machine = target.create_target_machine(&target_triple, "generic", "", opt, reloc, model).unwrap();

        // let path = Path::new("/tmp/some/path/main.asm");
        let path = Path::new("./main.elf");
        // let target = Target::from_name("x86-64").unwrap();
        // let target_machine = target.create_target_machine(
            // &TargetTriple::create("x86_64-pc-linux-gnu"),
            // "x86-64",
            // "", // "+avx2",
            // opt,
            // reloc,
            // model
        // )
        // .unwrap();
        self.module.set_data_layout(&target_machine.get_target_data().get_data_layout());
        self.module.set_triple(&target_triple);

        let void_type = self.context.void_type();
        let fn_type = void_type.fn_type(&[], false);

        self.module.add_function("my_fn", fn_type, None);

        let i8_type = self.context.i8_type();
        let i64_type = self.context.i64_type();
        let char_star_array_type = i64_type.into();
        // TODO: Learn from https://gota7.github.io/GotaGuide/ProgrammingLanguage/Llvm.html which does
        // a raw _start.
        let main_fn_type = i8_type.fn_type(&[i64_type.into(), char_star_array_type], false);
        let main = self.module.add_function("main", main_fn_type, None);

        let basic_block = self.context.append_basic_block(main, "entry");
        let builder = self.context.create_builder();
        builder.position_at_end(basic_block);

        let argc = main.get_nth_param(0)?.into_int_value();
        // let argv = main.get_nth_param(1)?.into_int_value();

        // self.printf("ARGC %d. ARGV[0]='%s'", &[argc.into(), argv.into()]);
        //
        let s = "WORLD";
        dbg!(s);
        let s = self.builder.build_global_string_ptr(&("HI ".to_string() + s), "my_str").as_pointer_value();
        dbg!(s);
        // let printf = self.get_printf();
        let printf_type = self.context.i32_type().fn_type(&[self.char_ptr_type().into()], true);
        let printf = self.module.add_function("printf", printf_type, None);
        dbg!(printf);
        self.builder.build_call(printf, &[s.into()], "_call_printf");
        dbg!("HERE");
        builder.build_return(Some(&argc));

        /*
        if false {
            let start_fn_type = void_type.fn_type(&[], false);
            let start = self.module.add_function("_start", start_fn_type, None);
            let basic_block = self.context.append_basic_block(start, "entry");
            let builder = self.context.create_builder();
            builder.position_at_end(basic_block);
            // https://stackoverflow.com/questions/16721164/x86-linux-assembler-get-program-parameters-from-start
            // let dummy_argc_val = i64_type.const_int(2, false);
            // let dummy_argv_val = i64_type.const_int(8000, false);
            // builder.build_call(main, &[dummy_argc_val.into(), dummy_argv_val.into()], "call_main");
            let sys_exit_val = u64_type.const_int(SYS_exit.try_into().unwrap(), false); // TODO: more wat
            let exit_status = i64_type.const_int(11, false);

            // SYSCALL
            let void_type = self.context.void_type();
            let fn_type = void_type.fn_type(&[], false);
            let syscall_wrapper = self.module.add_function("syscall_wrapper", fn_type, None);
            let basic_block = self.context.append_basic_block(syscall_wrapper, "entry");

            builder.position_at_end(basic_block);
            let asm_fn = self.context.i64_type().fn_type(&[context.i64_type().into(), self.context.i64_type().into()], false);
            let asm = self.context.create_inline_asm(
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
    */

        // Can compile this to elf with
        // clang main.bc -o main.elf -target x86_64-pc-linux-gnu
        // assert!(module.write_bitcode_to_path(Path::new("./main.bc")));

        assert!(target_machine.write_to_file(&self.module, FileType::Object, path).is_ok());

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
}

fn main() -> Result<(), Box<dyn Error>> {
        let context = Context::create();
        let module = context.create_module("sum");
        // let execution_engine = module.create_jit_execution_engine(OptimizationLevel::None)?;
        //
        //
        let mut codegen = CodeGen {
            context: &context,
            module,
            builder: context.create_builder(),
            // execution_engine,
            strings: HashMap::new(),
        };

    /*
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
    println!("STARTING!");
    codegen.write_machine();
    println!("FINISHED!");
    Ok(())
}
