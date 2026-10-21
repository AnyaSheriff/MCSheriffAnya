//! Общая граница синхронного чтения: предмет → улей → моб → предмет.
//! Публичные читатели вызывают друг друга, поэтому новый вызов не сбрасывает
//! глубину. Счётчик принадлежит потоку; RAII возвращает его при любом выходе.
use std::cell::Cell;

thread_local! {
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

pub(crate) struct Scope { _thread: std::marker::PhantomData<std::rc::Rc<()>> }

pub(crate) fn enter() -> Option<Scope> {
    DEPTH.with(|depth| {
        let current = depth.get();
        if current >= 16 { return None; }
        depth.set(current + 1);
        Some(Scope { _thread: std::marker::PhantomData })
    })
}

impl Drop for Scope {
    fn drop(&mut self) {
        DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}
