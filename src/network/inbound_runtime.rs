//! Один исполнитель чтения на процесс: игровые World/Items lock его не занимают.
use std::io;
use std::sync::OnceLock;
use tokio::runtime::{Handle, Runtime};

pub(super) fn handle() -> io::Result<&'static Handle> {
    // Статический Runtime живёт до завершения процесса. На подключение создаётся
    // только async reader; дополнительные потоки на каждого игрока не нужны.
    static IO: OnceLock<Result<Runtime, io::Error>> = OnceLock::new();
    match IO.get_or_init(|| tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .thread_name("mcsa-inbound-io")
        // Поток наследует affinity сетевого worker. Повторный расчёт cores
        // по уже урезанной маске ошибочно убрал бы ещё одно ядро.
        .enable_all()
        .build()) {
        Ok(runtime) => Ok(runtime.handle()),
        Err(error) => Err(io::Error::new(error.kind(), format!("Исполнитель чтения не запущен: {error}"))),
    }
}
