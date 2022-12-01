//#![feature(naked_functions)]
use core::time;
use goblin::elf::*;
use goblin::strtab::Strtab;
use libc::{self, c_void};
use std::arch::asm;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::prelude::*;
use std::os::unix::prelude::FileExt;

#[derive(Debug)]
struct Range {
    start: usize,
    end: usize,
}

/*
#[naked]
unsafe extern "C" fn gtod_trampoline() {
    const fptr: u64 = 64; //(my_gettimeofday as *const ()) as u64;
    asm!("mov rax, {0}", "jmp rax", in(reg) fptr, options(noreturn));
}
*/

extern "C" fn my_gettimeofday(tp: *mut libc::timeval, tz: *mut c_void) {
    unsafe {
        (*tp).tv_sec = 999;
        (*tp).tv_usec = 1234;
    }
}

fn vdso_mem_range() -> Result<Range, Box<dyn Error>> {
    let data = fs::read_to_string("/proc/self/maps")?;
    for line in data.lines() {
        if line.contains("vdso") {
            println!("{}", line);
            let (range, _) = line.split_once(" ").unwrap();
            let (start, end) = range.split_once("-").unwrap();
            return Ok(Range {
                start: usize::from_str_radix(start, 16).unwrap(),
                end: usize::from_str_radix(end, 16).unwrap(),
            });
        }
    }
    return Err("Blah".into());
}

pub fn curse_vdso() {
    let r = vdso_mem_range().unwrap();
    unsafe {
        libc::mprotect(
            r.start as *mut libc::c_void,
            r.end - r.start,
            libc::PROT_EXEC | libc::PROT_WRITE | libc::PROT_READ,
        );
    }
    let b = read_vdso(&r);
    mess_vdso(b, &r);
    vdso_mem_range().unwrap();
}

fn get_str_til_nul(s: &Strtab, at: usize) -> String {
    let mut ret: String = "".to_string();
    for c in s.get_at(at).unwrap().bytes() {
        if c == 0 {
            break;
        }
        ret.push(c.into());
    }
    return ret;
}
fn read_vdso(range: &Range) -> Vec<u8> {
    let mut buf = vec![0; range.end - range.start];
    let f = File::open("/proc/self/mem").unwrap();
    f.read_at(&mut buf, range.start as u64).unwrap();
    drop(f);
    return buf;
}

fn write_vdso(buf: &Vec<u8>) {
    let mut tmp = File::create("out").unwrap();
    tmp.write_all(&buf).unwrap();
}

fn overwrite(range: &Range, address: u64, size: u64) {
    let addr = (range.start as u64) + address;
    println!("Writing, addr {:x}", addr);
    unsafe {
        std::ptr::write_bytes((addr + 0) as *mut u8, 0xC3, 1); // RET
        std::ptr::write_bytes((addr + 1) as *mut u8, 0x90, (size - 1) as usize);
        // NOP
        /*
        std::ptr::write_bytes((addr + 0) as *mut u8, 0x48, 1);
        std::ptr::write_bytes((addr + 1) as *mut u8, 0xB8, 1);
        std::ptr::write_bytes((addr + 2) as *mut u8, ((addr >> 0) & 0xFF) as u8, 1);
        std::ptr::write_bytes((addr + 3) as *mut u8, ((addr >> 8) & 0xFF) as u8, 1);
        std::ptr::write_bytes((addr + 4) as *mut u8, ((addr >> 16) & 0xFF) as u8, 1);
        std::ptr::write_bytes((addr + 5) as *mut u8, ((addr >> 24) & 0xFF) as u8, 1);
        std::ptr::write_bytes((addr + 6) as *mut u8, ((addr >> 32) & 0xFF) as u8, 1);
        std::ptr::write_bytes((addr + 7) as *mut u8, ((addr >> 40) & 0xFF) as u8, 1);
        std::ptr::write_bytes((addr + 8) as *mut u8, ((addr >> 48) & 0xFF) as u8, 1);
        std::ptr::write_bytes((addr + 9) as *mut u8, ((addr >> 56) & 0xFF) as u8, 1);
        std::ptr::write_bytes((addr + 10) as *mut u8, 0xFF, 1);
        std::ptr::write_bytes((addr + 11) as *mut u8, 0xE0, 1);
        std::ptr::write_bytes((addr + 12) as *mut u8, 0x90, 2);
        */
    }
}
fn mess_vdso(buf: Vec<u8>, range: &Range) {
    println!("BEFORE WRITE {:?}", std::time::SystemTime::now());
    let r = Elf::parse(&buf).unwrap();
    // let r = object::File::parse(&*buf).unwrap();

    let mut va = 0;
    for s in r.program_headers {
        println!("head {:?} {}", s, s.p_type);
        if s.p_type == 2 {
            // "PT_DYNAMIC"
            va = s.p_vaddr;
        }
    }
    assert_ne!(va, 0);

    for s in r.section_headers {
        println!("sech {:?}", s);
    }
    println!("dynstr {:?}", r.dynstrtab);
    for s in &r.syms {
        println!("sym {:?}", s);
    }
    let mut address = 0;
    for ds in &r.dynsyms {
        println!(
            "dyns {:?} {:?}",
            ds,
            get_str_til_nul(&r.dynstrtab, ds.st_name)
        );
        if get_str_til_nul(&r.dynstrtab, ds.st_name).contains("time")
            || get_str_til_nul(&r.dynstrtab, ds.st_name).contains("clock")
        {
            address = ds.st_value;
            overwrite(range, address, ds.st_size);
        }
        /*
        if get_str_til_nul(&r.dynstrtab, ds.st_name) == "gettimeofday" {
            println!("dyn {:#?}", ds);
            address = ds.st_value;
        }
        */
    }
    write_vdso(&read_vdso(range));
    assert_ne!(address, 0);

    let mut tv: libc::timeval = libc::timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    unsafe {
        libc::gettimeofday(&mut tv, std::ptr::null_mut());
    }

    println!("{} {}", tv.tv_sec, tv.tv_usec);

    // ------------------
    std::thread::sleep(std::time::Duration::from_millis(100));
    unsafe {
        libc::gettimeofday(&mut tv, std::ptr::null_mut());
    }
    println!("{} {}", tv.tv_sec, tv.tv_usec);

    // ------------------
    /*
    std::thread::sleep(std::time::Duration::from_millis(100));
    let fptr = ((range.start as u64) + address) as *const ();
    let code: extern "C" fn(tp: *mut libc::timeval, tz: *mut c_void) =
        unsafe { std::mem::transmute(fptr) };
    (code)(&mut tv, std::ptr::null_mut());
    println!("{} {}", tv.tv_sec, tv.tv_usec);
    */

    // ------------------
    std::thread::sleep(std::time::Duration::from_millis(100));
    let fptr = my_gettimeofday as *const ();
    let code: extern "C" fn(tp: *mut libc::timeval, tz: *mut c_void) =
        unsafe { std::mem::transmute(fptr) };
    (code)(&mut tv, std::ptr::null_mut());
    println!("{} {}", tv.tv_sec, tv.tv_usec);

    // ------------------
    // FIXME WRITE HERE
    // overwrite(range, address);
    /*
    std::thread::sleep(std::time::Duration::from_millis(100));
    let fptr = addr as *const ();
    let code: extern "C" fn(tp: *mut libc::timeval, tz: *mut c_void) =
        unsafe { std::mem::transmute(fptr) };
    println!("Calling");
    (code)(&mut tv, std::ptr::null_mut());
    println!("{} {}", tv.tv_sec, tv.tv_usec);
    */
    /*
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/proc/self/mem")
        .unwrap();
    //f.write_at(&vec![0; 16], (range.start as u64) + address + ) .unwrap();
    println!("Range is {:x} {:x}", range.start, range.end);
    println!("Writing at {:x}", va + address + (range.start as u64));
    f.write_at(&vec![0; 16], va + address + (range.start as u64))
        .unwrap();
    println!("AFTER WRITE {:?}", std::time::SystemTime::now());
    //println!("{:#?}", r.symbol_map());
    */
    // ------------------
    std::thread::sleep(std::time::Duration::from_millis(100));
    tv.tv_sec = 333;
    unsafe {
        libc::gettimeofday(&mut tv, std::ptr::null_mut());
    }
    println!("alive ?? {} {}", tv.tv_sec, tv.tv_usec);
    println!("AFTER WRITE {:?}", std::time::SystemTime::now());
}
pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
