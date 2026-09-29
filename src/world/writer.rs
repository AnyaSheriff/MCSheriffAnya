// Запись мира на диск — в своём потоке.
//
// Запись со сбросом на диск (`sync_data`) при занятом диске ждёт секундами:
// пока идёт сборка или копирование, один сброс может занять пять секунд.
// Такт в это время стоит бы под замком мира, и мир замирал (так и было —
// «Не успеваю! Сервер отстал на 5207 мс» в чёрном ящике). Поэтому такт
// только складывает байты и отдаёт их сюда, а пишет отдельный поток — как
// у оригинала, где чанки пишет свой рабочий поток ввода-вывода.
//
// Порядок записей сохраняется: чанки, потом файл мира — одной очередью, как
// и раньше в одном потоке. Пока чанк не лёг на диск, его байты лежат здесь,
// и чтение (`region::read_stored`) берёт их отсюда: выгруженный и тут же
// загруженный снова чанк не прочитается с диска старым.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, OnceLock};

use super::region;
use crate::log_error;

/// Чанк, ждущий записи: папка регионов и координаты чанка.
type Key = (PathBuf, i32, i32);

/// Что ещё не на диске.
static PENDING: OnceLock<Mutex<HashMap<Key, Arc<Vec<u8>>>>> = OnceLock::new();

/// Очередь потока записи.
#[cfg(not(test))]
static QUEUE: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();

/// Через сколько повторить запись, которая не удалась.
#[cfg(not(test))]
const RETRY: std::time::Duration = std::time::Duration::from_secs(2);

enum Job {
    /// Чанки одного региона.
    Chunks { directory: PathBuf, chunks: Vec<(i32, i32, Arc<Vec<u8>>)> },

    /// Файл целиком (level.dat, ticks.dat): как его записать.
    File { name: String, write: Box<dyn Fn() -> io::Result<()> + Send> },

    /// Всё, что было в очереди до этого, записано: сообщить.
    Flush(Sender<()>),
}

fn pending() -> &'static Mutex<HashMap<Key, Arc<Vec<u8>>>> {
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Байты чанка, ещё не дошедшие до диска.
pub fn waiting(directory: &Path, chunk_x: i32, chunk_z: i32) -> Option<Arc<Vec<u8>>> {
    pending()
        .lock()
        .expect("очередь записи захвачена другим потоком")
        .get(&(directory.to_path_buf(), chunk_x, chunk_z))
        .cloned()
}

/// Отдать чанки одного региона на запись.
pub fn write_chunks(directory: &Path, chunks: Vec<(i32, i32, Vec<u8>)>) {
    let chunks: Vec<(i32, i32, Arc<Vec<u8>>)> =
        chunks.into_iter().map(|(x, z, bytes)| (x, z, Arc::new(bytes))).collect();

    {
        let mut pending = pending().lock().expect("очередь записи захвачена другим потоком");

        for (x, z, bytes) in &chunks {
            pending.insert((directory.to_path_buf(), *x, *z), bytes.clone());
        }
    }

    submit(Job::Chunks { directory: directory.to_path_buf(), chunks });
}

/// Отдать на запись файл: `write` пишет его целиком.
pub fn write_file(name: &str, write: impl Fn() -> io::Result<()> + Send + 'static) {
    submit(Job::File { name: name.to_string(), write: Box::new(write) });
}

/// Дождаться, пока всё отданное ляжет на диск, — при остановке сервера.
#[cfg(not(test))]
pub fn flush() {
    let (done, wait) = mpsc::channel();

    submit(Job::Flush(done));

    let _ = wait.recv();
}

/// В проверках записи идут сразу, в том же потоке: ждать нечего. Застряла
/// запись — она подождёт следующей (`submit`), проверка не зависнет.
#[cfg(test)]
pub fn flush() {
    let (done, _wait) = mpsc::channel();

    submit(Job::Flush(done));
}

#[cfg(test)]
thread_local! {
    /// Записи, которые не удались, по порядку: в проверках повторяются при
    /// следующей записи, а не в цикле.
    static STUCK: std::cell::RefCell<std::collections::VecDeque<Job>> = Default::default();
}

/// В проверках пишем сразу: они смотрят на диск тут же, после записи.
#[cfg(test)]
fn submit(job: Job) {
    STUCK.with(|stuck| {
        let mut stuck = stuck.borrow_mut();

        stuck.push_back(job);

        while let Some(job) = stuck.front() {
            if let Err(e) = attempt(job) {
                log_error!("Не удалось записать мир: {} — повторю", e);
                break;
            }

            if let Some(job) = stuck.pop_front() {
                finish(job);
            }
        }
    });
}

#[cfg(not(test))]
fn submit(job: Job) {
    let queue = QUEUE.get_or_init(|| {
        let (sender, jobs) = mpsc::channel::<Job>();

        let started = std::thread::Builder::new().name("world writer".to_string()).spawn(move || {
            // Ядро такта — только такту.
            crate::tick::keep_off_tick_core();

            for job in jobs {
                run(job);
            }
        });

        if let Err(e) = started {
            log_error!("Не удалось завести поток записи мира: {}", e);
        }

        Mutex::new(sender)
    });

    let sent = queue.lock().expect("очередь записи захвачена другим потоком").send(job);

    // Поток записи пропал — пишем сами, лишь бы не потерять.
    if let Err(mpsc::SendError(job)) = sent {
        run(job);
    }
}

/// Исполняет одну запись. Не удалась — повторяет, пока не получится:
/// очередь стоит, чтобы файл мира не обогнал чанки, а байты чанков до тех
/// пор ждут в памяти.
#[cfg(not(test))]
fn run(job: Job) {
    while let Err(e) = attempt(&job) {
        log_error!("Не удалось записать мир: {} — повторю", e);
        std::thread::sleep(RETRY);
    }

    finish(job);
}

/// Одна попытка записи.
fn attempt(job: &Job) -> io::Result<()> {
    match job {
        Job::Chunks { directory, chunks } => {
            let batch: Vec<(i32, i32, &[u8])> = chunks.iter().map(|(x, z, bytes)| (*x, *z, bytes.as_slice())).collect();

            region::write_many(directory, &batch).map_err(|e| {
                let (region_x, region_z) = region::region_of(batch[0].0, batch[0].1);

                io::Error::new(e.kind(), format!("регион {} {} ({} чанков): {}", region_x, region_z, batch.len(), e))
            })
        }
        Job::File { name, write } => write().map_err(|e| io::Error::new(e.kind(), format!("{}: {}", name, e))),
        Job::Flush(_) => Ok(()),
    }
}

/// Запись удалась: записанные чанки больше не ждут — если за это время их
/// не сменила запись новее; ждущему сброса — ответ.
fn finish(job: Job) {
    match job {
        Job::Chunks { directory, chunks } => {
            let mut pending = pending().lock().expect("очередь записи захвачена другим потоком");

            for (x, z, bytes) in &chunks {
                let key = (directory.clone(), *x, *z);

                if pending.get(&key).is_some_and(|waiting| Arc::ptr_eq(waiting, bytes)) {
                    pending.remove(&key);
                }
            }
        }
        Job::File { .. } => {}
        Job::Flush(done) => {
            let _ = done.send(());
        }
    }
}
