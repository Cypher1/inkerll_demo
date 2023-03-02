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

    pub fn global_string(&mut self, value: &str) -> PointerValue<'ctx> {
        self.strings.get(value).copied().unwrap_or_else(|| {
            let ptr_value = self.builder.build_global_string_ptr(value, "global_string");
            let ptr = ptr_value.as_pointer_value();
            self.strings.insert(value.to_string(), ptr);
            ptr
        })
    }

    pub fn printf(&mut self, fmt: &str, args: &[BasicMetadataValueEnum<'ctx>]) {
        let printf = self.get_printf();
        println!("HERE1");
        let fmt_str = self.global_string(fmt);
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
        // Target::initialize_x86(&InitializationConfig::default());
        Target::initialize_all(&InitializationConfig::default());
        let target_triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&target_triple).unwrap();
        let reloc = RelocMode::Default;
        let model = CodeModel::Default;
        let opt = OptimizationLevel::Default;
        let target_machine = target.create_target_machine(&target_triple, "generic", "", opt, reloc, model).unwrap();

        let path = Path::new("./main.elf");
        self.module.set_data_layout(&target_machine.get_target_data().get_data_layout());
        self.module.set_triple(&target_triple);

        let i8_type = self.context.i8_type();
        let i64_type = self.context.i64_type();
        let char_star_type = i8_type.ptr_type(AddressSpace::default());
        let char_star_array_type = char_star_type.ptr_type(AddressSpace::default());
        let main_fn_type = i8_type.fn_type(&[i64_type.into(), char_star_array_type.into()], false);
        let main = self.module.add_function("main", main_fn_type, None);

        let basic_block = self.context.append_basic_block(main, "entry");
        self.builder.position_at_end(basic_block);

        let argc = main.get_nth_param(0)?.into_int_value();
        let argv = main.get_nth_param(1)?;
        // let z = self.context.i64_type().const_int(0, false);
        // let hello_world = self.global_string("Hello World");
        let argv_0 = self.builder.build_load(char_star_type, argv.into_pointer_value(), "argv_0"); // load(argv, z, "argv_0");
        self.printf("ARGC: %d, ARGV: %s\n", &[argc.into(), argv_0.into()]);
        self.builder.build_return(Some(&argc));

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
