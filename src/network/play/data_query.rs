//! Чтение живого Health: никакого ожидания внутри задачи игрока.
use super::*;
use crate::player_query::{Failure, Identity, Request, Response};

pub(super) struct Pending { name: String, target: Identity, response: Response }

pub(super) fn accept(shared: &Shared, player: &mut PlayerState, target: Identity, name: String) -> Result<(), Failure> {
    if player.queries.len() >= 64 { return Err(Failure::Full); }
    let response = crate::player_query::enqueue(shared, target)?;
    player.queries.push(Pending { name, target, response });
    Ok(())
}

/// Вызывается ТОЛЬКО после обычного vitals::tick. Hurt в очереди ещё не HP.
/// Снимок относится к этому safe point, не обещает порядок между двумя Orders.
pub(super) fn finish_reads(reads: &mut Vec<Arc<Request>>, shared: &Shared, player: &PlayerState) {
    if reads.is_empty() { return; }
    let owner = Identity { uuid: player.uuid, entity_id: player.entity_id };
    let active = shared.players.lock().expect("игроки").members().iter()
        .any(|m| m.uuid == owner.uuid && m.entity_id == owner.entity_id);
    for request in reads.drain(..) {
        if active { request.complete(owner, player.vit.health); }
        // Иначе последний Arc закрывает канал; новому сеансу запрос не передаётся.
    }
}

/// Ответы остаются на исходном сокете: reconnect не наследует старый запрос.
pub(super) async fn send_ready(stream: &mut Outgoing<'_>, player: &mut PlayerState) -> io::Result<()> {
    let mut answers = Vec::new();
    player.queries.retain_mut(|pending| {
        if let Some(result) = pending.response.poll() {
            let result = match result {
                Ok(snapshot) if snapshot.owner != pending.target => Err(Failure::Disconnected),
                result => result,
            };
            answers.push(commands::player_health_answer(&pending.name, result)); false
        } else { true }
    });
    for answer in answers {
        log_info!("[{}: {}]", player.name, answer.reply);
        if let Some(component) = answer.component {
            let mut body = encode_varint(SYSTEM_CHAT);
            body.extend_from_slice(&component); body.push(0);
            write_body(stream, body).await?;
        } else { send_system_chat(stream, &answer.reply).await?; }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay_catalog::{Catalog, Profile};
    use std::sync::atomic::{AtomicUsize, Ordering};
    fn scene() -> (Shared, PlayerState, PlayerState) {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!("mcsa-live-query-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap();
        let catalog = Catalog::new(Profile::Preview26_3);
        let shared = Shared::new(crate::world::World::in_memory_with_catalog(catalog),crate::world::World::in_memory_with_catalog(catalog),
            Default::default(),Default::default(),Default::default(),crate::ops::Ops::open(&root.join("ops.json")),crate::skins::Settings::default());
        let make = |n, name: &str| {
            let mut p = PlayerState::new_for_catalog([n;16],name.into(),None,None,0x7f,u64::from(n),(0.5,65.,0.5),catalog);
            p.entity_id=i32::from(n);p.game_mode=GAME_MODE_SURVIVAL;p.abilities=crate::playerdata::Abilities::for_mode(GAME_MODE_SURVIVAL);p
        };
        let a=make(1,"Reader");let b=make(2,"Target");
        for p in [&a,&b] {
            shared.players.lock().unwrap().join(p.member(),p.entity_id).unwrap();
            shared.players.lock().unwrap().waker(&p.uuid);
            shared.ops.lock().unwrap().give(&p.uuid,&p.name,crate::ops::HIGHEST_LEVEL);
            publish_player(&shared,p);
        }
        std::fs::remove_dir_all(root).unwrap();(shared,a,b)
    }
    fn identity(p: &PlayerState) -> Identity { Identity { uuid:p.uuid,entity_id:p.entity_id } }
    fn command(p: &mut PlayerState, shared: &Shared, target: &str) -> Reaction {
        let mut body=encode_varint(SERVERBOUND_CHAT_COMMAND);body.extend(encode_string(&format!("data get entity {target} Health")));
        handle_client_packet(&PacketInfo { id:SERVERBOUND_CHAT_COMMAND,length:body.len() as i32,body,
            teleport:None,punch:false,foreign_inventory:false,legacy_inventory:None },p,shared)
    }
    fn requests(shared: &Shared, p: &PlayerState) -> Vec<Arc<Request>> {
        take_orders(shared,&p.uuid).into_iter().map(|o|match o {Order::PlayerQuery(r)=>r,_=>panic!("неожиданное поручение")}).collect()
    }
    #[tokio::test]
    async fn self_command_returns_before_owner_snapshot_and_keeps_exact_fraction() {
        let (shared,mut a,_)=scene();a.vit.health=19.5;
        let reaction=command(&mut a,&shared,"@s");assert!(reaction.reply.is_none());assert_eq!(a.queries.len(),1);
        let mut reads=requests(&shared,&a);assert_eq!(reads.len(),1);assert!(a.queries[0].response.poll().is_none());
        finish_reads(&mut reads,&shared,&a);assert!(reads.is_empty());
        let answer=a.queries[0].response.poll().unwrap().unwrap();assert_eq!(answer.health,19.5);assert_eq!(answer.owner,identity(&a));
    }
    #[tokio::test]
    async fn queued_hurt_is_read_after_real_vitals_not_published_gear() {
        let (shared,_,mut target)=scene();
        let mut response=crate::player_query::enqueue(&shared,identity(&target)).unwrap();let mut reads=requests(&shared,&target);
        target.net.hits.push(vitals::Pending { amount:6.625,source:crate::survival::Source::Generic,direct:0,cause:0,hurt_yaw:None,killer:None,push:None,announced:false,quiet_thorns:false });
        assert_eq!(target.vit.health,20.);assert!(response.poll().is_none());
        let mut out=Outgoing::capture();vitals::tick(&mut out,&shared,&mut target).await.unwrap();
        assert_eq!(target.vit.health,13.375);finish_reads(&mut reads,&shared,&target);
        assert_eq!(response.poll().unwrap().unwrap().health,13.375);
    }
    #[tokio::test]
    async fn mutual_queries_do_not_wait_inside_either_actor_and_reply_on_original_socket() {
        let (shared,mut a,mut b)=scene();a.vit.health=19.5;b.vit.health=13.375;
        command(&mut a,&shared,"Target");command(&mut b,&shared,"Reader");
        let mut ar=requests(&shared,&a);let mut br=requests(&shared,&b);
        finish_reads(&mut br,&shared,&b);finish_reads(&mut ar,&shared,&a);
        let mut ao=Outgoing::capture();let mut bo=Outgoing::capture();send_ready(&mut ao,&mut a).await.unwrap();send_ready(&mut bo,&mut b).await.unwrap();
        assert!(ao.queued().windows(b"13.375f".len()).any(|w|w==b"13.375f"));
        assert!(bo.queued().windows(b"19.5f".len()).any(|w|w==b"19.5f"));assert!(a.queries.is_empty()&&b.queries.is_empty());
    }
    #[tokio::test]
    async fn disconnect_and_same_uuid_rejoin_cannot_answer_old_ticket() {
        let (shared,_,old)=scene();let mut response=crate::player_query::enqueue(&shared,identity(&old)).unwrap();let mut reads=requests(&shared,&old);
        shared.players.lock().unwrap().leave(&old.uuid);let mut fresh=old.member();fresh.entity_id=100;
        shared.players.lock().unwrap().join(fresh,100).unwrap();
        finish_reads(&mut reads,&shared,&old);assert_eq!(response.poll(),Some(Err(Failure::Disconnected)));
        assert!(matches!(crate::player_query::enqueue(&shared,identity(&old)),Err(Failure::Disconnected)));
    }
    #[tokio::test(start_paused=true)]
    async fn absent_vitals_boundary_times_out_without_fabricating_health() {
        let (shared,mut a,_)=scene();command(&mut a,&shared,"@s");let reads=requests(&shared,&a);
        tokio::time::advance(Duration::from_secs(2)).await;
        assert_eq!(a.queries[0].response.poll(),Some(Err(Failure::Timeout)));assert!(!reads[0].pending());
    }
    #[tokio::test]
    async fn target_without_java_waker_is_rejected_before_enqueue() {
        let (shared,_,a)=scene();shared.players.lock().unwrap().leave(&a.uuid);
        shared.players.lock().unwrap().join(a.member(),a.entity_id).unwrap();
        assert!(matches!(crate::player_query::enqueue(&shared,identity(&a)),Err(Failure::Unsupported)));
        assert!(take_orders(&shared,&a.uuid).is_empty());
    }
    #[tokio::test]
    async fn requester_queue_has_a_bound_even_if_completed_tickets_are_not_polled() {
        let (shared,mut a,b)=scene();
        for _ in 0..64 {command(&mut a,&shared,"Target");let mut reads=requests(&shared,&b);finish_reads(&mut reads,&shared,&b);}
        assert_eq!(a.queries.len(),64);let last=command(&mut a,&shared,"Target");
        assert!(last.reply.unwrap().starts_with("Слишком много"));assert!(requests(&shared,&b).is_empty());
    }
}

#[cfg(test)]
mod mailbox_tests {
    use super::*;
    #[tokio::test(start_paused=true)]
    async fn cancelled_and_expired_queries_are_pruned_before_mailbox_append() {
        let catalog=crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3);
        let shared=Shared::new(crate::world::World::in_memory_with_catalog(catalog),crate::world::World::in_memory_with_catalog(catalog),Default::default(),Default::default(),Default::default(),crate::ops::Ops::open(&std::env::temp_dir().join(format!("mcsa-query-mailbox-ops-missing-{}.json",std::process::id()))),crate::skins::Settings::default());
        let mut player=PlayerState::new_for_catalog([7;16],"Mail".into(),None,None,0x7f,1,(0.,65.,0.),catalog);player.entity_id=7;
        shared.players.lock().unwrap().join(player.member(),7).unwrap();shared.players.lock().unwrap().waker(&player.uuid);
        shared.players.lock().unwrap().order(&player.uuid,Order::Clear);
        let id=Identity {uuid:player.uuid,entity_id:7};
        for _ in 0..200 {drop(crate::player_query::enqueue(&shared,id).unwrap());}
        let expired=crate::player_query::enqueue(&shared,id).unwrap();tokio::time::advance(Duration::from_secs(2)).await;
        let live=crate::player_query::enqueue(&shared,id).unwrap();
        let orders=take_orders(&shared,&player.uuid);assert_eq!(orders.len(),2);assert!(matches!(orders[0],Order::Clear));assert!(matches!(orders[1],Order::PlayerQuery(_)));
        drop((expired,live));
    }
}
