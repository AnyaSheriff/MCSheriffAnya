//! Стабильный ABI загрузчика. Ядро по-прежнему работает в отдельном процессе,
//! с прежними stdin/stdout, рабочей папкой, Tokio и штатным сохранением мира.
use std::{ffi::c_char, io, panic::{UnwindSafe, catch_unwind}};

const fn identifier(text: &str) -> [u8; 128] {
    let bytes=text.as_bytes();let mut result=[0;128];let mut index=0;
    while index<bytes.len()&&index+1<result.len() { result[index]=bytes[index];index+=1; }
    result
}

static BUILD_ID: [u8;128]=identifier(match option_env!("MCSA_CORE_BUILD_ID") { Some(value)=>value,None=>"local" });

#[unsafe(no_mangle)]
pub extern "C" fn mcsa_core_api_v1() -> u32 { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn mcsa_core_build_id_v1() -> *const c_char { BUILD_ID.as_ptr().cast() }

fn enter(run: impl FnOnce()->io::Result<()>+UnwindSafe) -> i32 {
    // Не выпускаем Rust panic через границу C. Процесс целиком завершится;
    // не пытаемся продолжить работу частично запущенного сервера.
    match catch_unwind(run) {
        Ok(Ok(()))=>0,
        Ok(Err(error))=>{eprintln!("MCSheriffAnya: не удалось запустить ядро: {error}");1},
        Err(_)=>{eprintln!("MCSheriffAnya: аварийная остановка ядра");101},
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mcsa_core_main_v1() -> i32 { enter(crate::main) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_stable_interface_and_nul_terminated_identifier() {
        assert_eq!(mcsa_core_api_v1(),1);
        let actual=unsafe { std::ffi::CStr::from_ptr(mcsa_core_build_id_v1()) };
        assert_eq!(actual.to_str().unwrap(),option_env!("MCSA_CORE_BUILD_ID").unwrap_or("local"));
        let long=identifier(&"a".repeat(300));assert_eq!(long[127],0);assert!(long[..127].iter().all(|&v|v==b'a'));
    }
    #[test]
    fn startup_errors_and_panics_do_not_escape_into_native_host() {
        assert_eq!(enter(||Ok(())),0);
        assert_eq!(enter(||Err(io::Error::other("test startup failure"))),1);
        assert_eq!(enter(||panic!("test panic boundary")),101);
    }
}
