//! Ограниченные одноразовые запросы здоровья игрока.

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::time::Instant;

/// Предел одновременно ожидающих билетов.
const CAPACITY: usize = 64;
/// Общий абсолютный срок запроса.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Identity {
    pub uuid: [u8; 16],
    pub entity_id: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Snapshot {
    pub owner: Identity,
    pub health: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Failure {
    Full,
    Disconnected,
    Timeout,
    Invalid,
    Unsupported,
}

/// Реестр слабых ссылок; сам запросы не удерживает.
#[derive(Default, Debug)]
pub(crate) struct Broker {
    tickets: Mutex<Vec<Weak<Request>>>,
}

#[derive(Debug)]
pub struct Request {
    target: Identity,
    deadline: Instant,
    sender: Mutex<Option<oneshot::Sender<Result<Snapshot, Failure>>>>,
}

pub(crate) struct Response {
    receiver: oneshot::Receiver<Result<Snapshot, Failure>>,
    deadline: Instant,
    consumed: bool,
}

impl Broker {
    /// Создаёт билет, попутно освобождая места закрытых, завершённых и просроченных.
    pub(crate) fn request(&self, target: Identity) -> Result<(Arc<Request>, Response), Failure> {
        let (sender, receiver) = oneshot::channel();
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        let request = Arc::new(Request {
            target,
            deadline,
            sender: Mutex::new(Some(sender)),
        });

        let mut tickets = self.tickets.lock().unwrap();
        tickets.retain(|ticket| match ticket.upgrade() {
            Some(request) => request.pending(),
            None => false,
        });

        if tickets.len() >= CAPACITY {
            return Err(Failure::Full);
        }

        tickets.push(Arc::downgrade(&request));
        drop(tickets);

        Ok((
            request,
            Response {
                receiver,
                deadline,
                consumed: false,
            },
        ))
    }
}

impl Request {
    pub(crate) fn target(&self) -> Identity {
        self.target
    }

    /// Запрос ещё может получить ответ и не истёк.
    pub(crate) fn pending(&self) -> bool {
        if Instant::now() >= self.deadline {
            return false;
        }
        let sender = self.sender.lock().unwrap();
        match sender.as_ref() {
            Some(sender) => !sender.is_closed(),
            None => false,
        }
    }

    /// Завершает запрос ровно один раз; true только при реально доставленном успехе.
    pub(crate) fn complete(&self, owner: Identity, health: f32) -> bool {
        let sender = {
            self.sender.lock().unwrap().take()
        };
        let sender = match sender {
            Some(sender) => sender,
            None => return false,
        };

        if Instant::now() >= self.deadline {
            let _ = sender.send(Err(Failure::Timeout));
            return false;
        }
        if owner != self.target {
            let _ = sender.send(Err(Failure::Disconnected));
            return false;
        }
        if !health.is_finite() || health < 0.0 {
            let _ = sender.send(Err(Failure::Invalid));
            return false;
        }

        sender.send(Ok(Snapshot { owner, health })).is_ok()
    }
}

impl Response {
    /// Неблокирующее чтение; Some возвращается только один раз.
    pub(crate) fn poll(&mut self) -> Option<Result<Snapshot, Failure>> {
        if self.consumed {
            return None;
        }
        if Instant::now() >= self.deadline {
            self.consumed = true;
            return Some(Err(Failure::Timeout));
        }

        match self.receiver.try_recv() {
            Ok(result) => {
                self.consumed = true;
                Some(result)
            }
            Err(oneshot::error::TryRecvError::Empty) => None,
            Err(oneshot::error::TryRecvError::Closed) => {
                self.consumed = true;
                Some(Err(Failure::Disconnected))
            }
        }
    }

    /// Ожидает ответ до общего абсолютного срока запроса.
    pub(crate) async fn wait(mut self) -> Result<Snapshot, Failure> {
        if self.consumed {
            return Err(Failure::Disconnected);
        }
        if Instant::now() >= self.deadline {
            return Err(Failure::Timeout);
        }

        match tokio::time::timeout_at(self.deadline, &mut self.receiver).await {
            Ok(Ok(result)) => {
                if Instant::now() >= self.deadline {
                    Err(Failure::Timeout)
                } else {
                    result
                }
            }
            Ok(Err(_)) => {
                if Instant::now() >= self.deadline {
                    Err(Failure::Timeout)
                } else {
                    Err(Failure::Disconnected)
                }
            }
            Err(_) => Err(Failure::Timeout),
        }
    }
}

/// Проверка подключения и постановка чтения атомарны относительно leave/rejoin.
/// Ни ожиданий, ни чтения Gear/UI/диска; очередь обрабатывает сам владелец.
pub(crate) fn enqueue(shared: &crate::shared::Shared, target: Identity) -> Result<Response, Failure> {
    if shared.catalog().profile() != crate::gameplay_catalog::Profile::Preview26_3 { return Err(Failure::Unsupported); }
    let mut players = shared.players.lock().expect("игроки");
    if !players.members().iter().any(|m| m.uuid == target.uuid && m.entity_id == target.entity_id) {
        return Err(Failure::Disconnected);
    }
    if !players.has_java_waker(&target.uuid) { return Err(Failure::Unsupported); }
    players.prune_player_queries(&target.uuid);
    let (request, response) = shared.player_queries.request(target)?;
    let uuid = request.target().uuid;
    players.order(&uuid, crate::players::Order::PlayerQuery(request));
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time;

    fn id(byte: u8, entity_id: i32) -> Identity {
        Identity {
            uuid: [byte; 16],
            entity_id,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn complete_then_poll_once() {
        let broker = Broker::default();
        let owner = id(1, 10);
        let (request, mut response) = broker.request(owner).unwrap();

        assert_eq!(request.target(), owner);
        assert!(request.pending());
        assert_eq!(response.poll(), None);

        assert!(request.complete(owner, 20.0));
        assert!(!request.pending());
        assert_eq!(
            response.poll(),
            Some(Ok(Snapshot {
                owner,
                health: 20.0
            }))
        );
        assert_eq!(response.poll(), None);
    }

    #[tokio::test(start_paused = true)]
    async fn independent_tickets_do_not_cross_talk() {
        let broker = Broker::default();
        let first = id(2, 1);
        let second = id(3, 2);
        let (first_request, mut first_response) = broker.request(first).unwrap();
        let (second_request, mut second_response) = broker.request(second).unwrap();

        assert!(second_request.complete(second, 30.5));
        assert!(first_request.complete(first, 10.0));

        assert_eq!(
            first_response.poll(),
            Some(Ok(Snapshot {
                owner: first,
                health: 10.0
            }))
        );
        assert_eq!(
            second_response.poll(),
            Some(Ok(Snapshot {
                owner: second,
                health: 30.5
            }))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn exact_owner_mismatch_disconnects() {
        let broker = Broker::default();
        let target = id(4, 7);

        let (request, mut response) = broker.request(target).unwrap();
        let wrong_uuid = Identity {
            uuid: [9; 16],
            entity_id: target.entity_id,
        };
        assert!(!request.complete(wrong_uuid, 1.0));
        assert_eq!(response.poll(), Some(Err(Failure::Disconnected)));

        let (request, mut response) = broker.request(target).unwrap();
        let wrong_entity = Identity {
            uuid: target.uuid,
            entity_id: target.entity_id + 1,
        };
        assert!(!request.complete(wrong_entity, 1.0));
        assert_eq!(response.poll(), Some(Err(Failure::Disconnected)));
    }

    #[tokio::test(start_paused = true)]
    async fn repeat_completion_is_ignored() {
        let broker = Broker::default();
        let owner = id(5, 5);
        let (request, mut response) = broker.request(owner).unwrap();

        assert!(request.complete(owner, 12.0));
        assert!(!request.complete(owner, 99.0));
        assert_eq!(
            response.poll(),
            Some(Ok(Snapshot {
                owner,
                health: 12.0
            }))
        );
        assert!(!request.complete(owner, 1.0));
        assert_eq!(response.poll(), None);
    }

    #[tokio::test(start_paused = true)]
    async fn invalid_health_is_rejected() {
        let broker = Broker::default();
        let owner = id(6, 6);

        for health in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.5] {
            let (request, mut response) = broker.request(owner).unwrap();
            assert!(!request.complete(owner, health));
            assert_eq!(response.poll(), Some(Err(Failure::Invalid)));
        }

        let (request, mut response) = broker.request(owner).unwrap();
        assert!(request.complete(owner, -0.0));
        match response.poll() {
            Some(Ok(snapshot)) => {
                assert_eq!(snapshot.owner, owner);
                assert!(snapshot.health == 0.0);
                assert!(snapshot.health.is_sign_negative());
            }
            other => panic!("ожидался успешный снимок: {:?}", other),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn receiver_drop_cancels() {
        let broker = Broker::default();
        let owner = id(7, 7);
        let (request, response) = broker.request(owner).unwrap();

        assert!(request.pending());
        drop(response);
        assert!(!request.pending());
        assert!(!request.complete(owner, 10.0));
    }

    #[tokio::test(start_paused = true)]
    async fn last_request_drop_disconnects() {
        let broker = Broker::default();
        let owner = id(8, 8);
        let (request, mut response) = broker.request(owner).unwrap();

        drop(request);
        assert_eq!(response.poll(), Some(Err(Failure::Disconnected)));
        assert_eq!(response.poll(), None);
    }

    #[tokio::test(start_paused = true)]
    async fn capacity_is_bounded_to_sixty_four() {
        let broker = Broker::default();
        let owner = id(9, 9);
        let mut held = Vec::new();

        for _ in 0..CAPACITY {
            held.push(broker.request(owner).unwrap());
        }
        assert!(matches!(broker.request(owner), Err(Failure::Full)));

        drop(held.pop());
        assert!(broker.request(owner).is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_reclaims_capacity_with_old_request_alive() {
        let broker = Broker::default();
        let owner = id(10, 10);
        let mut requests = Vec::new();
        let mut responses = Vec::new();

        for _ in 0..CAPACITY {
            let (request, response) = broker.request(owner).unwrap();
            requests.push(request);
            responses.push(response);
        }
        assert!(matches!(broker.request(owner), Err(Failure::Full)));

        drop(responses);
        assert!(broker.request(owner).is_ok());
        assert_eq!(requests.len(), CAPACITY);
    }

    #[tokio::test(start_paused = true)]
    async fn timeout_reclaims_capacity_with_old_request_alive() {
        let broker = Broker::default();
        let owner = id(11, 11);
        let mut requests = Vec::new();
        let mut responses = Vec::new();

        for _ in 0..CAPACITY {
            let (request, response) = broker.request(owner).unwrap();
            requests.push(request);
            responses.push(response);
        }
        assert!(matches!(broker.request(owner), Err(Failure::Full)));

        time::advance(Duration::from_secs(2)).await;
        assert!(broker.request(owner).is_ok());
        assert_eq!(requests.len(), CAPACITY);
        assert_eq!(responses.len(), CAPACITY);
    }

    #[tokio::test(start_paused = true)]
    async fn completed_unread_survives_broker_prune() {
        let broker = Broker::default();
        let owner = id(12, 12);
        let (request, mut response) = broker.request(owner).unwrap();

        assert!(request.complete(owner, 44.0));
        let _other = broker.request(id(13, 13)).unwrap();

        assert_eq!(
            response.poll(),
            Some(Ok(Snapshot {
                owner,
                health: 44.0
            }))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn strict_deadline_wins_over_queued_response() {
        let broker = Broker::default();
        let owner = id(14, 14);
        let (request, mut response) = broker.request(owner).unwrap();

        assert!(request.complete(owner, 33.0));
        time::advance(Duration::from_secs(2)).await;

        assert_eq!(response.poll(), Some(Err(Failure::Timeout)));
        assert_eq!(response.poll(), None);
    }

    #[tokio::test(start_paused = true)]
    async fn wait_success() {
        let broker = Broker::default();
        let owner = id(15, 15);
        let (request, response) = broker.request(owner).unwrap();
        assert!(request.complete(owner, 7.0));

        assert_eq!(
            response.wait().await,
            Ok(Snapshot {
                owner,
                health: 7.0
            })
        );
    }

    #[tokio::test(start_paused = true)]
    async fn wait_times_out_at_deadline() {
        let broker = Broker::default();
        let owner = id(16, 16);
        let (_request, response) = broker.request(owner).unwrap();

        assert_eq!(response.wait().await, Err(Failure::Timeout));
    }

    #[tokio::test(start_paused = true)]
    async fn wait_disconnected_when_last_request_drops() {
        let broker = Broker::default();
        let owner = id(17, 17);
        let (request, response) = broker.request(owner).unwrap();

        drop(request);
        assert_eq!(response.wait().await, Err(Failure::Disconnected));
    }

    #[tokio::test(start_paused = true)]
    async fn wait_does_not_deliver_after_deadline() {
        let broker = Broker::default();
        let owner = id(18, 18);
        let (request, response) = broker.request(owner).unwrap();
        assert!(request.complete(owner, 5.0));

        time::advance(Duration::from_secs(2)).await;
        assert_eq!(response.wait().await, Err(Failure::Timeout));
    }
}
