//! Отдельный ignored-сервер для реального клиента. В release такой точки входа нет.
use super::*;

#[tokio::test(flavor="multi_thread",worker_threads=2)]
#[ignore="изолированная игровая сессия777, запуск только внешней пробой"]
async fn serve_java_26_3_probe() {
    // Работаем только в отдельной временной папке, которую подготовила проба.
    assert!(Path::new("mcsa-gameplay-probe.marker").is_file());
    let properties=ServerProperties {online_mode:false,server_port:26026,view_distance:2,simulation_distance:2,compression_threshold:-1,..Default::default()};
    let mut world=World::in_memory();
    for z in -24..24 {for x in -24..24 {world.set_block(x,63,z,blocks::state_by_name("stone").unwrap());}}
    let mut shared=Shared::new(world,World::in_memory(),PathBuf::from("world/playerdata"),PathBuf::from("skins"),properties.clone(),
        ops::Ops::open(Path::new("config/ops.json")),skins::Settings {mojang:false,ely:false,..Default::default()});
    shared.settings.java_version=Some(mcsa_protocol::JavaVersion::V26_3);
    shared.settings.auth_enabled=false;shared.settings.auth_premium=false;shared.settings.prefetch_chunks=0;
    let shared=Arc::new(shared);let properties=Arc::new(properties);
    network::packet::set_compression_threshold(-1);
    let listener=TcpListener::bind("127.0.0.1:26026").await.unwrap();
    let (sender,mut shutdown)=watch::channel(false);
    console::start(Arc::clone(&shared),sender);tick::start(Arc::clone(&shared));
    println!("MCSA gameplay probe ready on 26026");
    let mut tasks=tokio::task::JoinSet::new();
    loop {tokio::select!{
        accepted=listener.accept()=>{
            let (socket,addr)=accepted.unwrap();socket.set_nodelay(true).unwrap();
            let shared=Arc::clone(&shared);let properties=Arc::clone(&properties);
            tasks.spawn(async move {if let Err(error)=handle_connection(socket,addr,properties,shared).await {eprintln!("MCSA probe connection: {error}");}});
        }
        _=shutdown.changed()=>break,
    }}
    kick_everyone(&shared).await;tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    tasks.abort_all();
}
