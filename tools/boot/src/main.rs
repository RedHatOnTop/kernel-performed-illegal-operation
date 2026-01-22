//! KPIO 커널 디스크 이미지 빌더
//!
//! bootloader 크레이트를 사용하여 UEFI 및 BIOS 부팅 가능한
//! 디스크 이미지를 생성합니다.
//!
//! # 사용법
//!
//! ```bash
//! # 커널 먼저 빌드
//! cargo build --release -p kpio-kernel
//!
//! # tools/boot 디렉토리에서 이미지 빌더 실행
//! cd tools/boot
//! cargo run --release -- ../../target/x86_64-unknown-none/release/kernel
//! ```

use bootloader::DiskImageBuilder;
use std::path::PathBuf;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <kernel-binary-path>", args[0]);
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} ../../target/x86_64-unknown-none/release/kernel", args[0]);
        std::process::exit(1);
    }
    
    let kernel_path = PathBuf::from(&args[1]);
    
    if !kernel_path.exists() {
        eprintln!("Error: Kernel binary not found at: {}", kernel_path.display());
        eprintln!();
        eprintln!("Make sure to build the kernel first:");
        eprintln!("  cargo build --release -p kpio-kernel");
        std::process::exit(1);
    }
    
    println!("Building disk images from kernel: {}", kernel_path.display());
    
    // 출력 디렉토리 결정
    let output_dir = kernel_path.parent().unwrap_or_else(|| {
        eprintln!("Error: Cannot determine output directory");
        std::process::exit(1);
    });
    
    let uefi_path = output_dir.join("kpio-uefi.img");
    let bios_path = output_dir.join("kpio-bios.img");
    
    // 디스크 이미지 빌더 생성
    let builder = DiskImageBuilder::new(kernel_path.clone());
    
    // UEFI 이미지 생성
    print!("Creating UEFI image... ");
    match builder.create_uefi_image(&uefi_path) {
        Ok(_) => println!("OK"),
        Err(e) => {
            println!("FAILED");
            eprintln!("Error creating UEFI image: {}", e);
            std::process::exit(1);
        }
    }
    
    // BIOS 이미지 생성
    print!("Creating BIOS image... ");
    match builder.create_bios_image(&bios_path) {
        Ok(_) => println!("OK"),
        Err(e) => {
            println!("FAILED");
            eprintln!("Error creating BIOS image: {}", e);
            std::process::exit(1);
        }
    }
    
    println!();
    println!("Disk images created:");
    println!("  UEFI: {}", uefi_path.display());
    println!("  BIOS: {}", bios_path.display());
    println!();
    println!("Run with QEMU (UEFI):");
    println!("  qemu-system-x86_64 -bios OVMF.fd -drive format=raw,file={} -serial stdio", 
             uefi_path.display());
    println!();
    println!("Run with QEMU (BIOS):");
    println!("  qemu-system-x86_64 -drive format=raw,file={} -serial stdio", 
             bios_path.display());
}
