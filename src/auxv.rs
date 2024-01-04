use std::error::Error;

#[derive(Debug)]
pub struct AuxVecValues {
    pub(crate) vdso_base: usize,
    pub(crate) page_size: usize,
}

extern "C" {
    static environ: *const *const u8;
}

unsafe fn get_auxv_ptr() -> *const usize {
    // the auxiliary vector is right behind the environment variables, which
    // is an array of strings, delimited by a nullpointer.
    let mut env_entry_ptr = environ;

    while !(*env_entry_ptr).is_null() {
        env_entry_ptr = env_entry_ptr.offset(1);
    }

    env_entry_ptr = env_entry_ptr.offset(1);

    return std::mem::transmute::<*const *const u8, *const usize>(env_entry_ptr);
}

pub(crate) fn read_aux_vec() -> Result<AuxVecValues, Box<dyn Error>> {
    // The auxiliary vector is an array of key:value tuples, represented as [usize, usize]
    // The end is delimited by having the key == AT_NULL
    let mut out = unsafe { get_auxv_ptr() };
    let mut ptr = 0;
    let mut pagesize = 0;
    unsafe {
        while *out != libc::AT_NULL as usize {
            let key = *out;
            let val = *out.offset(1);
            if key == libc::AT_SYSINFO_EHDR as usize {
                ptr = val;
            }
            if key == libc::AT_PAGESZ as usize  {
                pagesize = val;
            }
            out = out.offset(2);
        }
    }
    if ptr == 0 || pagesize == 0 {
        panic!("wtf");
    }
    Ok(AuxVecValues {vdso_base: ptr, page_size: pagesize})
}

