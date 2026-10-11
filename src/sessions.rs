// Общий для Java и Bedrock порядок смены подключения: сначала уход и
// сохранение прежнего игрока, затем чтение файла новым подключением.

use std::sync::{Arc, Mutex, Weak};
use tokio::sync::watch;

#[derive(Clone, Debug)]
pub struct Disconnect {
    pub component: Vec<u8>,
    pub plain: String,
}

impl Disconnect {
    pub fn duplicate() -> Self {
        // minecraft.wiki/w/Server: "You logged in from another location".
        // Передаём ключ перевода, чтобы Java показала причину на языке клиента.
        Self {
            component: crate::network::nbt::translated("multiplayer.disconnect.duplicate_login"),
            plain: "You logged in from another location".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct Control(watch::Sender<Option<Disconnect>>);

impl Default for Control {
    fn default() -> Self { Self(watch::channel(None).0) }
}

impl Control {
    pub fn disconnect(&self, reason: Disconnect) {
        self.0.send_if_modified(|current| {
            if current.is_some() { return false; }
            *current = Some(reason);
            true
        });
    }

    pub fn reason(&self) -> Option<Disconnect> { self.0.borrow().clone() }

    pub async fn cancelled(&self) {
        let mut receiver = self.0.subscribe();
        let _ = receiver.wait_for(Option::is_some).await;
    }
}

struct Active {
    name: String,
    uuid: [u8; 16],
    control: Control,
    finished: watch::Sender<bool>,
}

/// Живёт до конца сохранения, удаления из списка и отписки от журналов.
pub struct Lease(Arc<Active>);

impl Lease {
    pub fn control(&self) -> Control { self.0.control.clone() }
}

impl Drop for Lease {
    fn drop(&mut self) { self.0.finished.send_replace(true); }
}

#[derive(Default)]
pub struct Sessions(Mutex<Vec<Weak<Active>>>);

impl Sessions {
    pub async fn acquire(&self, name: &str, uuid: [u8; 16]) -> Lease {
        loop {
            let previous = {
                let mut sessions = self.0.lock().expect("подключения захвачены другим потоком");
                sessions.retain(|entry| entry.upgrade().is_some_and(|entry| !*entry.finished.borrow()));
                let previous: Vec<_> = sessions.iter().filter_map(Weak::upgrade)
                    .filter(|entry| entry.name.eq_ignore_ascii_case(name) || entry.uuid == uuid)
                    .map(|entry| {
                        entry.control.disconnect(Disconnect::duplicate());
                        entry.finished.subscribe()
                    }).collect();
                if previous.is_empty() {
                    let entry = Arc::new(Active {
                        name: name.to_string(), uuid, control: Control::default(),
                        finished: watch::channel(false).0,
                    });
                    sessions.push(Arc::downgrade(&entry));
                    return Lease(entry);
                }
                previous
            };
            // Замок списка здесь отпущен: уход и чужие входы могут идти.
            for mut finished in previous {
                let _ = finished.wait_for(|done| *done).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn same_name_or_uuid_waits_for_old_cleanup_and_other_players_do_not() {
        let sessions = Sessions::default();
        let first = sessions.acquire("SheriffAnya", [1; 16]).await;
        let control = first.control();
        let replacement = sessions.acquire("sheriffanya", [2; 16]);
        tokio::pin!(replacement);
        tokio::select! {
            biased;
            _ = &mut replacement => panic!("вошёл до сохранения"),
            _ = control.cancelled() => {}
        }
        assert!(control.reason().unwrap().component.windows(38).any(|s| s == b"multiplayer.disconnect.duplicate_login"));
        let other = sessions.acquire("Other", [3; 16]).await;
        drop(first);
        let second = replacement.await;
        let control = second.control();
        let renamed = sessions.acquire("Renamed", [2; 16]);
        tokio::pin!(renamed);
        tokio::select! {
            biased;
            _ = &mut renamed => panic!("тот же файл занят"),
            _ = control.cancelled() => {}
        }
        assert!(other.control().reason().is_none());
        drop(second);
        drop(renamed.await);
    }

    #[tokio::test]
    async fn concurrent_replacements_never_own_the_same_player_together() {
        let sessions = Sessions::default();
        let first = sessions.acquire("Player", [1; 16]).await;
        let a = sessions.acquire("PLAYER", [1; 16]);
        let b = sessions.acquire("player", [1; 16]);
        tokio::pin!(a, b);
        let control = first.control();
        tokio::select! {
            biased;
            _ = &mut a => panic!("прежний ещё владеет"),
            _ = &mut b => panic!("прежний ещё владеет"),
            _ = control.cancelled() => {}
        }
        drop(first);
        let second = a.await;
        let control = second.control();
        tokio::select! {
            biased;
            _ = &mut b => panic!("новый уже владеет"),
            _ = control.cancelled() => {}
        }
        drop(second);
        drop(b.await);
    }
}
